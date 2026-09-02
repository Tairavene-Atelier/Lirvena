use std::collections::BTreeSet;

use qq_envelope::{QqTeaKey, encrypt_qq_tea};
use qq_profile::LinuxNtProfile;
use qq_wire::{LengthPrefix, WireReader, WireWriter};
use zeroize::Zeroize;

use crate::qr_packet::packet::{build_request_data, build_transaction, build_wtlogin_packet};
use crate::qr_packet::response::decode_wtlogin_body;
use crate::{
    QqKeyAgreement, QrChallenge, QrPacketError, QrPollState, QrResponseContext, QrUnsignedRequest,
};

#[cfg(test)]
mod tests;

const QR_POLL_COMMAND: u16 = 0x12;
const MAX_LOGIN_PACKET_LEN: usize = 64 * 1024;
const MAX_TLV_COUNT: usize = 64;
const MAX_TLV_LEN: usize = 32 * 1024;

/// Borrowed inputs for one QR polling request.
#[derive(Clone, Copy)]
pub struct QrPollContext<'a> {
    /// Selected validated Linux profile.
    pub profile: &'a LinuxNtProfile,
    /// SSO request sequence.
    pub sso_sequence: u32,
    /// Current Unix timestamp in seconds.
    pub unix_seconds: u32,
    /// Per-installation random login header value.
    pub random_key: &'a QqTeaKey,
    /// Ephemeral login key agreement.
    pub key_agreement: &'a dyn QqKeyAgreement,
    /// Challenge returned by the matching fetch response.
    pub challenge: &'a QrChallenge,
}

/// Zeroizing temporary credentials returned after QR confirmation.
pub struct QrLoginSecrets {
    uin: u64,
    tgtgt_key: Box<[u8]>,
    temporary_password: Box<[u8]>,
    no_picture_signature: Box<[u8]>,
}

impl QrLoginSecrets {
    #[cfg(test)]
    pub(crate) fn for_test(
        uin: u64,
        tgtgt_key: Vec<u8>,
        temporary_password: Vec<u8>,
        no_picture_signature: Vec<u8>,
    ) -> Self {
        Self {
            uin,
            tgtgt_key: tgtgt_key.into_boxed_slice(),
            temporary_password: temporary_password.into_boxed_slice(),
            no_picture_signature: no_picture_signature.into_boxed_slice(),
        }
    }

    /// Returns the confirmed numeric QQ account identifier.
    #[must_use]
    pub const fn uin(&self) -> u64 {
        self.uin
    }

    /// Borrows the temporary TGTGT key.
    #[must_use]
    pub fn tgtgt_key(&self) -> &[u8] {
        &self.tgtgt_key
    }

    /// Borrows the temporary password material.
    #[must_use]
    pub fn temporary_password(&self) -> &[u8] {
        &self.temporary_password
    }

    /// Borrows the no-picture signature.
    #[must_use]
    pub fn no_picture_signature(&self) -> &[u8] {
        &self.no_picture_signature
    }
}

impl core::fmt::Debug for QrLoginSecrets {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("QrLoginSecrets")
            .field("tgtgt_key", &"<redacted>")
            .field("temporary_password", &"<redacted>")
            .field("no_picture_signature", &"<redacted>")
            .finish()
    }
}

impl Drop for QrLoginSecrets {
    fn drop(&mut self) {
        self.tgtgt_key.zeroize();
        self.temporary_password.zeroize();
        self.no_picture_signature.zeroize();
    }
}

/// Closed QR polling response.
pub enum QrPollResponse {
    /// QR login remains pending or ended without credentials.
    State(QrPollState),
    /// QR confirmation produced all required temporary credentials.
    Confirmed(QrLoginSecrets),
}

impl core::fmt::Debug for QrPollResponse {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::State(state) => formatter.debug_tuple("State").field(state).finish(),
            Self::Confirmed(secrets) => formatter.debug_tuple("Confirmed").field(secrets).finish(),
        }
    }
}

/// Builds one encrypted QR polling body for the ordinary signing boundary.
///
/// # Errors
///
/// Returns an error for invalid sequences, empty challenge, bounds or encryption failure.
pub fn build_qr_poll(context: QrPollContext<'_>) -> Result<QrUnsignedRequest, QrPacketError> {
    if context.sso_sequence == 0
        || context.unix_seconds == 0
        || context.challenge.poll_signature().is_empty()
        || context.key_agreement.public_key().len() != 25
    {
        return Err(QrPacketError::InvalidField);
    }
    let body = build_poll_body(context.profile, context.challenge)?;
    let transaction = build_transaction(QR_POLL_COMMAND, &body)?;
    let data = build_request_data(context.profile, context.unix_seconds, &transaction)?;
    let encrypted = encrypt_qq_tea(&data, context.key_agreement.tea_key())?;
    let payload = build_wtlogin_packet(
        context.profile,
        context.random_key,
        context.key_agreement.public_key(),
        &encrypted,
    )?;
    Ok(QrUnsignedRequest::new(context.sso_sequence, payload))
}

/// Decrypts and validates one QR polling response.
///
/// # Errors
///
/// Returns an error for invalid state values, mismatched bindings or incomplete credentials.
pub fn decode_qr_poll_response(
    payload: &[u8],
    context: QrResponseContext<'_>,
) -> Result<QrPollResponse, QrPacketError> {
    let body = decode_wtlogin_body(payload, context, QR_POLL_COMMAND)?;
    let mut reader = WireReader::new(&body);
    if reader.read_u16()? != 0 || reader.read_u32()? != context.app_id {
        return Err(QrPacketError::InvalidField);
    }
    let state =
        QrPollState::try_from(reader.read_u8()?).map_err(|_error| QrPacketError::InvalidField)?;
    if state != QrPollState::Confirmed {
        // The 52194 implementation consumes only the state for pending and terminal
        // responses. QQ may append ordinary generation-specific fields here; the
        // enclosing login packet has already been authenticated, decrypted and
        // bounded, so they are intentionally not interpreted by this runtime ABI.
        return Ok(QrPollResponse::State(state));
    }
    let uin = reader.read_u64()?;
    let _retry = reader.read_i32()?;
    if uin == 0 {
        return Err(QrPacketError::InvalidField);
    }
    let secrets = parse_credentials(&mut reader, uin)?;
    reader.finish()?;
    Ok(QrPollResponse::Confirmed(secrets))
}

fn build_poll_body(
    profile: &LinuxNtProfile,
    challenge: &QrChallenge,
) -> Result<Vec<u8>, QrPacketError> {
    let mut body = WireWriter::new(MAX_LOGIN_PACKET_LEN);
    body.put_u16(0)?;
    body.put_u32(profile.app_id())?;
    body.put_prefixed_bytes(LengthPrefix::U16Payload, challenge.poll_signature())?;
    body.put_u64(0)?;
    body.put_u8(0)?;
    body.put_prefixed_bytes(LengthPrefix::U16Payload, &[])?;
    body.put_u16(0)?;
    Ok(body.finish())
}

fn parse_credentials(
    reader: &mut WireReader<'_>,
    uin: u64,
) -> Result<QrLoginSecrets, QrPacketError> {
    let count = usize::from(reader.read_u16()?);
    if count == 0 || count > MAX_TLV_COUNT {
        return Err(QrPacketError::InvalidField);
    }
    let mut tags = BTreeSet::new();
    let mut tgtgt_key = None;
    let mut temporary_password = None;
    let mut no_picture_signature = None;
    for _index in 0..count {
        let tag = reader.read_u16()?;
        if !tags.insert(tag) {
            return Err(QrPacketError::InvalidField);
        }
        let body = reader
            .read_prefixed_bytes(LengthPrefix::U16Payload, MAX_TLV_LEN)?
            .to_vec()
            .into_boxed_slice();
        match tag {
            0x01e => tgtgt_key = Some(body),
            0x018 => temporary_password = Some(body),
            0x019 => no_picture_signature = Some(body),
            _ => {}
        }
    }
    let secrets = QrLoginSecrets {
        uin,
        tgtgt_key: tgtgt_key.ok_or(QrPacketError::InvalidField)?,
        temporary_password: temporary_password.ok_or(QrPacketError::InvalidField)?,
        no_picture_signature: no_picture_signature.ok_or(QrPacketError::InvalidField)?,
    };
    if secrets.tgtgt_key.is_empty()
        || secrets.temporary_password.is_empty()
        || secrets.no_picture_signature.is_empty()
    {
        return Err(QrPacketError::InvalidField);
    }
    Ok(secrets)
}
