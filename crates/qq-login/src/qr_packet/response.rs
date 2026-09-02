use std::collections::BTreeSet;

use prost::Message;
use qq_envelope::{QqTeaKey, decrypt_qq_tea};
use qq_wire::{LengthPrefix, WireReader};
use zeroize::Zeroize;

use crate::{QqKeyAgreement, QrArtifact, QrPacketError};

const WTLOGIN_VERSION: u16 = 8_001;
const WTLOGIN_COMMAND: u16 = 2_066;
const QR_FETCH_COMMAND: u16 = 0x31;
const MAX_LOGIN_PACKET_LEN: usize = 64 * 1024;
const MAX_TLV_COUNT: usize = 64;
const MAX_TLV_LEN: usize = 32 * 1024;
const MAX_CHALLENGE_LEN: usize = 2_048;

#[derive(Clone, PartialEq, Message)]
struct QrResponseInfo {
    #[prost(string, tag = "2")]
    url: String,
    #[prost(string, tag = "3")]
    query_signature: String,
}

/// Expected local bindings for one QR fetch response.
#[derive(Clone, Copy)]
pub struct QrResponseContext<'a> {
    /// Expected ordinary application identifier.
    pub app_id: u32,
    /// Local response receipt time.
    pub issued_at_ms: u64,
    /// Per-installation random login key used by one response state.
    pub random_key: &'a QqTeaKey,
    /// Key agreement used by the matching request.
    pub key_agreement: &'a dyn QqKeyAgreement,
}

/// Sensitive QR values retained only for later polling and login exchange.
pub struct QrChallenge {
    poll_signature: Box<[u8]>,
    query_signature: String,
}

impl QrChallenge {
    #[cfg(test)]
    pub(super) fn for_test(poll_signature: Vec<u8>, query_signature: String) -> Self {
        Self {
            poll_signature: poll_signature.into_boxed_slice(),
            query_signature,
        }
    }

    /// Borrows the binary poll signature.
    #[must_use]
    pub fn poll_signature(&self) -> &[u8] {
        &self.poll_signature
    }

    /// Borrows the query signature needed by the later login exchange.
    #[must_use]
    pub fn query_signature(&self) -> &str {
        &self.query_signature
    }
}

impl core::fmt::Debug for QrChallenge {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("QrChallenge")
            .field("poll_signature", &"<redacted>")
            .field("poll_signature_len", &self.poll_signature.len())
            .field("query_signature", &"<redacted>")
            .finish()
    }
}

impl Drop for QrChallenge {
    fn drop(&mut self) {
        self.poll_signature.zeroize();
        self.query_signature.zeroize();
    }
}

/// Validated initial QR response.
pub struct QrFetchResponse {
    artifact: QrArtifact,
    challenge: QrChallenge,
}

impl QrFetchResponse {
    /// Returns the display artifact.
    #[must_use]
    pub const fn artifact(&self) -> &QrArtifact {
        &self.artifact
    }

    /// Returns the sensitive continuation values.
    #[must_use]
    pub const fn challenge(&self) -> &QrChallenge {
        &self.challenge
    }

    /// Splits display and continuation values for separate ownership.
    #[must_use]
    pub fn into_parts(self) -> (QrArtifact, QrChallenge) {
        (self.artifact, self.challenge)
    }
}

impl core::fmt::Debug for QrFetchResponse {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("QrFetchResponse")
            .field("artifact", &self.artifact)
            .field("challenge", &self.challenge)
            .finish()
    }
}

/// Decrypts and validates one QR-fetch command response.
///
/// # Errors
///
/// Returns an error for mismatched sequence/profile, malformed TLVs or invalid artifacts.
pub fn decode_qr_fetch_response(
    payload: &[u8],
    context: QrResponseContext<'_>,
) -> Result<QrFetchResponse, QrPacketError> {
    let body = decode_wtlogin_body(payload, context, QR_FETCH_COMMAND)?;
    let mut reader = WireReader::new(&body);
    if reader.read_u16()? != 0 || reader.read_u32()? != context.app_id {
        return Err(QrPacketError::InvalidField);
    }
    let return_code = reader.read_u8()?;
    if return_code != 0 {
        return Err(QrPacketError::InvalidField);
    }
    let poll_signature = reader
        .read_prefixed_bytes(LengthPrefix::U16Payload, MAX_CHALLENGE_LEN)?
        .to_vec()
        .into_boxed_slice();
    if poll_signature.is_empty() {
        return Err(QrPacketError::InvalidField);
    }
    let (png, lifetime_seconds, info) = parse_fetch_tlvs(&mut reader)?;
    reader.finish()?;
    if info.query_signature.is_empty() || info.query_signature.len() > MAX_CHALLENGE_LEN {
        return Err(QrPacketError::InvalidField);
    }
    let artifact = QrArtifact::new(info.url, png, context.issued_at_ms, lifetime_seconds)
        .map_err(|_error| QrPacketError::InvalidField)?;
    Ok(QrFetchResponse {
        artifact,
        challenge: QrChallenge {
            poll_signature,
            query_signature: info.query_signature,
        },
    })
}

pub(super) fn decode_wtlogin_body(
    payload: &[u8],
    context: QrResponseContext<'_>,
    expected_code2d_command: u16,
) -> Result<Vec<u8>, QrPacketError> {
    if payload.len() > MAX_LOGIN_PACKET_LEN {
        return Err(QrPacketError::InvalidField);
    }
    let mut reader = WireReader::new(payload);
    if reader.read_u8()? != 2 {
        return Err(QrPacketError::InvalidField);
    }
    let internal_len = usize::from(reader.read_u16()?);
    if internal_len != payload.len() || reader.read_u16()? != WTLOGIN_VERSION {
        return Err(QrPacketError::InvalidField);
    }
    if reader.read_u16()? != WTLOGIN_COMMAND || reader.read_u16()? != 0 || reader.read_u32()? != 0 {
        return Err(QrPacketError::InvalidField);
    }
    let _flag = reader.read_u8()?;
    let encryption = reader.read_u8()?;
    let state = reader.read_u8()?;
    let encrypted_len = reader
        .remaining()
        .checked_sub(1)
        .ok_or(QrPacketError::InvalidField)?;
    let encrypted = reader.read_bytes(encrypted_len)?;
    if reader.read_u8()? != 3 {
        return Err(QrPacketError::InvalidField);
    }
    reader.finish()?;
    let code2d = match encryption {
        0 => {
            let response_key = if state == 180 {
                context.random_key
            } else {
                context.key_agreement.tea_key()
            };
            decrypt_qq_tea(encrypted, response_key)?
        }
        4 => decrypt_ephemeral_response(encrypted, context.key_agreement)?,
        _ => return Err(QrPacketError::InvalidField),
    };
    decode_code2d_body(&code2d, expected_code2d_command)
}

fn decrypt_ephemeral_response(
    encrypted: &[u8],
    key_agreement: &dyn QqKeyAgreement,
) -> Result<Vec<u8>, QrPacketError> {
    let outer = decrypt_qq_tea(encrypted, key_agreement.tea_key())?;
    let mut reader = WireReader::new(&outer);
    let peer_public = reader.read_prefixed_bytes(LengthPrefix::U16Payload, 256)?;
    if peer_public.is_empty() || reader.remaining() == 0 {
        return Err(QrPacketError::InvalidField);
    }
    let response_key = key_agreement
        .derive_response_key(peer_public)
        .map_err(|_error| QrPacketError::Crypto)?;
    let inner = reader.read_bytes(reader.remaining())?;
    reader.finish()?;
    decrypt_qq_tea(inner, &response_key).map_err(Into::into)
}

fn decode_code2d_body(payload: &[u8], expected_command: u16) -> Result<Vec<u8>, QrPacketError> {
    let mut outer = WireReader::new(payload);
    let _outer_flag = outer.read_u8()?;
    if outer.read_u8()? != 0 {
        return Err(QrPacketError::InvalidField);
    }
    let layer_len = usize::from(outer.read_u16()?);
    let _outer_state = outer.read_u8()?;
    let transaction = outer.read_bytes(layer_len)?;
    outer.finish()?;

    let mut transaction = WireReader::new(transaction);
    if transaction.read_u8()? != 2
        || usize::from(transaction.read_u16()?) != transaction.remaining() + 3
        || transaction.read_u16()? != expected_command
    {
        return Err(QrPacketError::InvalidField);
    }
    let _reserved = transaction.read_bytes(21)?;
    if transaction.read_u8()? != 3
        || transaction.read_u16()? != 0
        || transaction.read_u16()? != 0x32
        || transaction.read_u32()? != 0
        || transaction.read_u64()? != 0
    {
        return Err(QrPacketError::InvalidField);
    }
    let body_len = transaction
        .remaining()
        .checked_sub(1)
        .ok_or(QrPacketError::InvalidField)?;
    let body = transaction.read_bytes(body_len)?.to_vec();
    if transaction.read_u8()? != 3 {
        return Err(QrPacketError::InvalidField);
    }
    transaction.finish()?;
    Ok(body)
}

fn parse_fetch_tlvs(
    reader: &mut WireReader<'_>,
) -> Result<(Vec<u8>, u32, QrResponseInfo), QrPacketError> {
    let count = usize::from(reader.read_u16()?);
    if count == 0 || count > MAX_TLV_COUNT {
        return Err(QrPacketError::InvalidField);
    }
    let mut tags = BTreeSet::new();
    let mut png = None;
    let mut lifetime_seconds = None;
    let mut info = None;
    for _index in 0..count {
        let tag = reader.read_u16()?;
        if !tags.insert(tag) {
            return Err(QrPacketError::InvalidField);
        }
        let body = reader.read_prefixed_bytes(LengthPrefix::U16Payload, MAX_TLV_LEN)?;
        match tag {
            0x017 => png = Some(body.to_vec()),
            0x01c => lifetime_seconds = Some(parse_lifetime(body)?),
            0x0d1 => {
                info = Some(
                    QrResponseInfo::decode(body).map_err(|_error| QrPacketError::InvalidField)?,
                );
            }
            _ => {}
        }
    }
    Ok((
        png.ok_or(QrPacketError::InvalidField)?,
        lifetime_seconds.ok_or(QrPacketError::InvalidField)?,
        info.ok_or(QrPacketError::InvalidField)?,
    ))
}

fn parse_lifetime(body: &[u8]) -> Result<u32, QrPacketError> {
    let mut reader = WireReader::new(body);
    let seconds = reader.read_u32()?;
    let _minutes = reader.read_u16()?;
    reader.finish()?;
    if seconds == 0 {
        Err(QrPacketError::InvalidField)
    } else {
        Ok(seconds)
    }
}
