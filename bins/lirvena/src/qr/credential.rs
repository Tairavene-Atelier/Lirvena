use std::io;

use ceylith_client::InstallationClient;
use ceylith_protocol::AccountSlotId;
use qq_envelope::QqTeaKey;
use qq_login::{
    CredentialExchangeContext, CredentialExchangeOutcome, CredentialLogin,
    CredentialResponseContext, LinuxKeyAgreement, QrDevice, QrLoginSecrets,
    build_credential_exchange, decode_credential_exchange_response,
};
use qq_profile::LinuxNtProfile;
use qq_transport::QqTransport;
use tokio::net::TcpStream;

use super::ceylith::{OpaqueOperation, profile_peer};
use super::qq::execute_request;
use crate::support::{random_array, random_nonzero_u32};

pub(super) async fn exchange(
    ceylith: &InstallationClient,
    qq: &mut QqTransport<TcpStream>,
    profile: &LinuxNtProfile,
    device: &QrDevice,
    account_slot_id: AccountSlotId,
    qr_secrets: &QrLoginSecrets,
) -> Result<CredentialLogin, Box<dyn std::error::Error>> {
    let key_agreement = LinuxKeyAgreement::new(profile_peer(profile)?)?;
    let random_key = QqTeaKey::new(random_array()?);
    let request = build_credential_exchange(CredentialExchangeContext {
        profile,
        device,
        sso_sequence: random_nonzero_u32()?,
        random_key: &random_key,
        key_agreement: &key_agreement,
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
            key_agreement: &key_agreement,
            tgtgt_key: qr_secrets.tgtgt_key(),
        },
    )?;
    match outcome {
        CredentialExchangeOutcome::Success(login) => Ok(login),
        CredentialExchangeOutcome::Rejected(rejection) => Err(io::Error::other(format!(
            "QQ rejected credential exchange with state {}",
            rejection.state()
        ))
        .into()),
    }
}
