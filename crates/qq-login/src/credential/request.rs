use qq_envelope::{QqTeaKey, encrypt_qq_tea};
use qq_profile::LinuxNtProfile;
use qq_wire::{LengthPrefix, WireWriter};

use super::tlv::build_login_tlvs;
use crate::{CredentialExchangeError, QqKeyAgreement, QrDevice, QrLoginSecrets};

const MAX_LOGIN_PACKET_LEN: usize = 64 * 1024;
const WTLOGIN_COMMAND: u16 = 2_064;

#[cfg(test)]
mod tests;

/// Borrowed inputs for one post-QR credential exchange.
#[derive(Clone, Copy)]
pub struct CredentialExchangeContext<'a> {
    /// Selected signed Linux Profile.
    pub profile: &'a LinuxNtProfile,
    /// Stable local device values.
    pub device: &'a QrDevice,
    /// SSO request sequence.
    pub sso_sequence: u32,
    /// Per-login random header key.
    pub random_key: &'a QqTeaKey,
    /// Fresh key agreement for the authenticated login exchange.
    pub key_agreement: &'a dyn QqKeyAgreement,
    /// Temporary credentials returned by QR confirmation.
    pub secrets: &'a QrLoginSecrets,
}

/// Unsigned ordinary QQ login body awaiting a Ceylith reserve.
pub struct CredentialExchangeRequest {
    sequence: u32,
    uin: u32,
    payload: Vec<u8>,
}

impl CredentialExchangeRequest {
    /// Returns the SSO sequence bound into the request.
    #[must_use]
    pub const fn sequence(&self) -> u32 {
        self.sequence
    }

    /// Returns the confirmed QQ account identifier.
    #[must_use]
    pub const fn uin(&self) -> u32 {
        self.uin
    }

    /// Returns the ordinary QQ command label.
    #[must_use]
    pub const fn command(&self) -> &'static str {
        "wtlogin.login"
    }

    /// Returns the complete ordinary request body used at the signing boundary.
    #[must_use]
    pub fn payload(&self) -> &[u8] {
        &self.payload
    }
}

impl core::fmt::Debug for CredentialExchangeRequest {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("CredentialExchangeRequest")
            .field("sequence", &self.sequence)
            .field("uin", &self.uin)
            .field("payload_len", &self.payload.len())
            .finish()
    }
}

/// Builds the authenticated-login body after a QR confirmation.
///
/// # Errors
///
/// Returns an error for invalid identifiers, credentials, lengths or encryption.
pub fn build_credential_exchange(
    context: CredentialExchangeContext<'_>,
) -> Result<CredentialExchangeRequest, CredentialExchangeError> {
    let uin =
        u32::try_from(context.secrets.uin()).map_err(|_| CredentialExchangeError::InvalidField)?;
    if context.sso_sequence == 0
        || uin == 0
        || context.key_agreement.public_key().len() != 25
        || context.secrets.tgtgt_key().len() != QqTeaKey::LENGTH
    {
        return Err(CredentialExchangeError::InvalidField);
    }
    let mut plaintext = WireWriter::new(MAX_LOGIN_PACKET_LEN);
    plaintext.put_u16(0x09)?;
    plaintext.put_bytes(&build_login_tlvs(
        context.profile,
        context.device,
        context.secrets,
        uin,
    )?)?;
    let encrypted = encrypt_qq_tea(&plaintext.finish(), context.key_agreement.tea_key())?;
    let payload = build_packet(context, uin, &encrypted)?;
    Ok(CredentialExchangeRequest {
        sequence: context.sso_sequence,
        uin,
        payload,
    })
}

fn build_packet(
    context: CredentialExchangeContext<'_>,
    uin: u32,
    encrypted: &[u8],
) -> Result<Vec<u8>, CredentialExchangeError> {
    let mut body = WireWriter::new(MAX_LOGIN_PACKET_LEN);
    body.put_u16(8_001)?;
    body.put_u16(WTLOGIN_COMMAND)?;
    body.put_u16(0)?;
    body.put_u32(uin)?;
    body.put_u8(3)?;
    body.put_u8(135)?;
    body.put_u32(0)?;
    body.put_u8(19)?;
    body.put_u16(0)?;
    body.put_u16(context.profile.app_client_version())?;
    body.put_u32(0)?;
    body.put_u8(1)?;
    body.put_u8(1)?;
    body.put_bytes(context.random_key.as_bytes())?;
    body.put_u16(0x102)?;
    body.put_prefixed_bytes(LengthPrefix::U16Payload, context.key_agreement.public_key())?;
    body.put_bytes(encrypted)?;
    body.put_u8(3)?;
    let body = body.finish();
    let declared_len = body
        .len()
        .checked_add(3)
        .and_then(|length| u16::try_from(length).ok())
        .ok_or(CredentialExchangeError::InvalidField)?;
    let mut output = WireWriter::new(MAX_LOGIN_PACKET_LEN);
    output.put_u8(2)?;
    output.put_u16(declared_len)?;
    output.put_bytes(&body)?;
    Ok(output.finish())
}
