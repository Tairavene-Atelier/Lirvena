use std::io;
use std::time::Duration;

use ceylith_client::InstallationClient;
use ceylith_protocol::AccountSlotId;
use qq_domain::LoginMachine;
use qq_envelope::QqTeaKey;
use qq_login::{
    LinuxKeyAgreement, QrChallenge, QrDevice, QrLoginSecrets, QrPollContext, QrPollResponse,
    QrPollState, QrResponseContext, apply_qr_poll, build_qr_poll, decode_qr_poll_response,
};
use qq_profile::LinuxNtProfile;
use qq_transport::QqTransport;
use tokio::net::TcpStream;

use super::ceylith::OpaqueOperation;
use super::qq::execute_request;
use crate::support::{now_ms, now_seconds, random_nonzero_u32};

const QR_POLL_INTERVAL: Duration = Duration::from_secs(2);

#[derive(Clone, Copy)]
pub(super) struct QrPolling<'a> {
    pub profile: &'a LinuxNtProfile,
    pub device: &'a QrDevice,
    pub random_key: &'a QqTeaKey,
    pub key_agreement: &'a LinuxKeyAgreement,
    pub account_slot_id: AccountSlotId,
    pub challenge: &'a QrChallenge,
    pub expires_at_ms: u64,
}

pub(super) async fn until_confirmed(
    ceylith: &InstallationClient,
    qq: &mut QqTransport<TcpStream>,
    polling: QrPolling<'_>,
    login: &mut LoginMachine,
) -> Result<QrLoginSecrets, Box<dyn std::error::Error>> {
    loop {
        wait_for_poll().await?;
        if now_ms()? >= polling.expires_at_ms {
            let _event = apply_qr_poll(login, QrPollState::Expired, now_ms()?)?;
            return Err(io::Error::new(io::ErrorKind::TimedOut, "QQ QR code expired").into());
        }
        let response = poll_once(ceylith, qq, polling).await?;
        match response {
            QrPollResponse::Confirmed(secrets) => {
                let _event = apply_qr_poll(login, QrPollState::Confirmed, now_ms()?)?;
                return Ok(secrets);
            }
            QrPollResponse::State(state @ (QrPollState::Expired | QrPollState::Canceled)) => {
                let _event = apply_qr_poll(login, state, now_ms()?)?;
                return Err(io::Error::other(format!("QQ QR flow ended: {state:?}")).into());
            }
            QrPollResponse::State(state) => update_pending_state(login, state)?,
        }
    }
}

async fn wait_for_poll() -> Result<(), io::Error> {
    tokio::select! {
        () = tokio::time::sleep(QR_POLL_INTERVAL) => Ok(()),
        result = tokio::signal::ctrl_c() => {
            result?;
            Err(io::Error::new(io::ErrorKind::Interrupted, "QR polling stopped"))
        }
    }
}

async fn poll_once(
    ceylith: &InstallationClient,
    qq: &mut QqTransport<TcpStream>,
    polling: QrPolling<'_>,
) -> Result<QrPollResponse, Box<dyn std::error::Error>> {
    let unsigned = build_qr_poll(QrPollContext {
        profile: polling.profile,
        sso_sequence: random_nonzero_u32()?,
        unix_seconds: now_seconds()?,
        random_key: polling.random_key,
        key_agreement: polling.key_agreement,
        challenge: polling.challenge,
    })?;
    let payload = execute_request(
        ceylith,
        qq,
        polling.profile,
        polling.device,
        polling.account_slot_id,
        OpaqueOperation::A,
        &unsigned,
    )
    .await?;
    Ok(decode_qr_poll_response(
        &payload,
        QrResponseContext {
            app_id: polling.profile.app_id(),
            issued_at_ms: now_ms()?,
            random_key: polling.random_key,
            key_agreement: polling.key_agreement,
        },
    )?)
}

fn update_pending_state(login: &mut LoginMachine, state: QrPollState) -> Result<(), io::Error> {
    let previous = login.state();
    apply_qr_poll(login, state, now_ms()?).map_err(io::Error::other)?;
    if login.state() != previous {
        eprintln!("Lirvena QR state: {state:?}");
    }
    Ok(())
}
