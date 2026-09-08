use std::io;

use ceylith_client::InstallationClient;
use ceylith_protocol::AccountSlotId;
use qq_envelope::QqTeaKey;
use qq_login::{
    CredentialExchangeContext, CredentialExchangeOutcome, CredentialLogin,
    CredentialResponseContext, LinuxKeyAgreement, QrDevice, QrLoginSecrets, WtLoginSequence,
    build_credential_exchange, decode_credential_exchange_response,
};
use qq_profile::LinuxNtProfile;
use qq_transport::QqTransport;
use tokio::net::TcpStream;

use super::ceylith::OpaqueOperation;
use super::qq::execute_request;
use crate::support::random_nonzero_u32;

pub(super) struct CredentialFlow<'a> {
    pub(super) ceylith: &'a InstallationClient,
    pub(super) profile: &'a LinuxNtProfile,
    pub(super) device: &'a QrDevice,
    pub(super) account_slot_id: AccountSlotId,
    pub(super) qr_secrets: &'a QrLoginSecrets,
    pub(super) random_key: &'a QqTeaKey,
    pub(super) key_agreement: &'a LinuxKeyAgreement,
}

pub(super) async fn exchange(
    flow: CredentialFlow<'_>,
    qq: &mut QqTransport<TcpStream>,
    wtlogin_sequence: &mut WtLoginSequence,
) -> Result<CredentialLogin, Box<dyn std::error::Error>> {
    let CredentialFlow {
        ceylith,
        profile,
        device,
        account_slot_id,
        qr_secrets,
        random_key,
        key_agreement,
    } = flow;
    let request = build_credential_exchange(CredentialExchangeContext {
        profile,
        device,
        sso_sequence: random_nonzero_u32()?,
        wtlogin_sequence: wtlogin_sequence.take(),
        random_key,
        key_agreement,
        secrets: qr_secrets,
    })?;
    let payload = execute_request(
        ceylith,
        qq,
        profile,
        device,
        account_slot_id,
        OpaqueOperation::B,
        &request,
    )
    .await?;
    let outcome = decode_credential_exchange_response(
        &payload,
        CredentialResponseContext {
            uin: request.uin(),
            key_agreement,
            tgtgt_key: qr_secrets.tgtgt_key(),
        },
    )
    .map_err(|error| {
        io::Error::other(format!(
            "QQ credential response validation failed: {error:?}"
        ))
    })?;
    match outcome {
        CredentialExchangeOutcome::Success(login) => Ok(login),
        CredentialExchangeOutcome::Rejected(rejection) => {
            let tag = rejection.tag().unwrap_or("<none>");
            let message = rejection.message().unwrap_or("<none>");
            Err(io::Error::other(format!(
                "QQ rejected credential exchange with state {}, tag {tag:?}, message {message:?}",
                rejection.state()
            ))
            .into())
        }
    }
}
