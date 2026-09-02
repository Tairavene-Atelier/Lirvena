use account_api::{AccountActionReceiver, AccountEvent, AccountEventPublisher, AccountIdentity};
use account_runtime::{AccountHandle, AccountPhase, AccountTransition, AssignedRealm};
use ceylith_client::InstallationClient;
use ceylith_protocol::AccountSlotId;
use qq_domain::{LoginMachine, LoginState};
use qq_envelope::QqTeaKey;
use qq_login::{
    LinuxKeyAgreement, QrDevice, QrFetchContext, QrResponseContext, accept_qr_artifact,
    begin_qr_request, build_qr_fetch, decode_qr_fetch_response,
};
use qq_profile::LinuxNtProfile;
use qq_session::AuthenticatedSession;
use qq_transport::{QqEndpoint, QqTransport, TransportConfig};

use super::ceylith::{OpaqueOperation, profile_peer};
use super::credential::exchange;
use super::polling::{QrPolling, until_confirmed};
use super::qq::execute_request;
use crate::config::AccountConfig;
use crate::online::{OnlineContext, OnlineRuntime};
use crate::support::{encode_hex, now_ms, now_seconds, random_array, random_nonzero_u32};

pub(super) struct AccountFlow<'a> {
    pub(super) config: &'a AccountConfig,
    pub(super) state_directory: &'a Path,
    pub(super) ceylith: &'a InstallationClient,
    pub(super) profile: &'a LinuxNtProfile,
    pub(super) realm: AssignedRealm,
    pub(super) account: &'a AccountHandle,
    pub(super) events: &'a AccountEventPublisher,
}

pub(super) async fn run(
    flow: AccountFlow<'_>,
    actions: AccountActionReceiver,
) -> Result<(), Box<dyn std::error::Error>> {
    let AccountFlow {
        config,
        state_directory,
        ceylith,
        profile,
        realm,
        account,
        events,
    } = flow;
    let mut login = LoginMachine::new();
    let _event = begin_qr_request(&mut login, now_ms()?)?;
    let key_agreement = LinuxKeyAgreement::new(profile_peer(profile)?)?;
    let device = QrDevice::new(config.device.clone());
    let random_key = QqTeaKey::new(random_array()?);
    let unsigned = build_qr_fetch(QrFetchContext {
        profile,
        device: &device,
        sso_sequence: random_nonzero_u32()?,
        unix_seconds: now_seconds()?,
        random_key: &random_key,
        key_agreement: &key_agreement,
    })?;
    eprintln!("Lirvena prepared the QQ QR request");

    let mut qq = QqTransport::connect(QqEndpoint::Primary, TransportConfig::default()).await?;
    eprintln!("Lirvena connected to QQ");
    let account_slot_id = AccountSlotId::from_bytes(config.account_slot_id);
    let payload = execute_request(
        ceylith,
        &mut qq,
        profile,
        &device,
        account_slot_id,
        OpaqueOperation::A,
        &unsigned,
    )
    .await?;
    let response = decode_qr_fetch_response(
        &payload,
        QrResponseContext {
            app_id: profile.app_id(),
            issued_at_ms: now_ms()?,
            random_key: &random_key,
            key_agreement: &key_agreement,
        },
    )?;
    eprintln!("Lirvena decoded the QQ QR response");
    let (artifact, challenge) = response.into_parts();
    let _event = accept_qr_artifact(&mut login, &artifact, now_ms()?)?;
    tokio::fs::write(&config.qr_output_path, artifact.png()).await?;
    println!("{}", artifact.terminal_text()?);
    println!("Lirvena QR PNG: {}", config.qr_output_path.display());
    println!("Lirvena QR SHA-256: {}", encode_hex(&artifact.png_sha256()));
    println!("Lirvena QR expires at: {}", artifact.expires_at_ms());
    eprintln!("Lirvena QR session is polling; press Ctrl-C to stop.");

    let secrets = until_confirmed(
        ceylith,
        &mut qq,
        QrPolling {
            profile,
            device: &device,
            random_key: &random_key,
            key_agreement: &key_agreement,
            account_slot_id,
            challenge: &challenge,
            expires_at_ms: artifact.expires_at_ms(),
        },
        &mut login,
    )
    .await?;
    println!("Lirvena QR confirmed for QQ account {}", secrets.uin());
    let credential = exchange(
        ceylith,
        &mut qq,
        profile,
        &device,
        account_slot_id,
        &secrets,
    )
    .await?;
    let _previous = login.transition(LoginState::Registering)?;
    println!(
        "Lirvena accepted QQ credentials for {} ({})",
        credential.nickname(),
        credential.uid()
    );
    let identity = AccountIdentity::new(
        account.local_id(),
        secrets.uin(),
        credential.nickname().to_owned(),
    )?;
    let _delivered = events.publish(AccountEvent::IdentityReady(identity.clone()));
    let mut qq = AuthenticatedSession::new(qq);
    let mut online =
        OnlineRuntime::new(profile, &device, identity, events.clone(), state_directory)?;
    online
        .bootstrap(OnlineContext {
            ceylith,
            qq: &mut qq,
            profile,
            realm,
            device: &device,
            credential: &credential,
            uin: secrets.uin(),
            account_slot_id,
        })
        .await?;
    let occurred_at_ms = now_ms()?;
    account
        .transition(AccountTransition {
            next: AccountPhase::Active,
            protective_reason: None,
            occurred_at_ms,
        })
        .await?;
    let _delivered = events.publish(AccountEvent::Lifecycle {
        local_id: account.local_id(),
        phase: AccountPhase::Active,
        protective_reason: None,
        occurred_at_ms,
    });
    let _previous = login.transition(LoginState::Online)?;
    eprintln!("Lirvena completed the required online startup gates");
    eprintln!("Lirvena QQ session is online; press Ctrl-C to stop.");
    online
        .run(
            OnlineContext {
                ceylith,
                qq: &mut qq,
                profile,
                realm,
                device: &device,
                credential: &credential,
                uin: secrets.uin(),
                account_slot_id,
            },
            actions,
        )
        .await?;
    let _previous = login.transition(LoginState::Stopped)?;
    Ok(())
}
use std::path::Path;
