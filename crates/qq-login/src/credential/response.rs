use std::collections::BTreeMap;

use prost::Message;
use qq_envelope::{QqTeaKey, decrypt_qq_tea};
use qq_wire::{LengthPrefix, WireReader};

use crate::{
    CredentialExchangeError, CredentialExchangeOutcome, CredentialLogin, CredentialRejection,
    CredentialSessionSecrets, QqKeyAgreement,
};

const WTLOGIN_VERSION: u16 = 8_001;
const WTLOGIN_COMMAND: u16 = 2_064;
const INTERNAL_COMMAND: u16 = 0x09;
const MAX_LOGIN_PACKET_LEN: usize = 64 * 1024;
const MAX_TLV_COUNT: usize = 64;
const MAX_TLV_LEN: usize = 32 * 1024;
const MAX_NOTICE_LEN: usize = 2_048;
const MAX_UID_PROTO_LEN: usize = 8 * 1024;

#[cfg(test)]
mod tests;

#[derive(Clone, PartialEq, Message)]
struct UidEnvelope {
    #[prost(message, optional, tag = "9")]
    layer_one: Option<UidLayerOne>,
}

#[derive(Clone, PartialEq, Message)]
struct UidLayerOne {
    #[prost(message, optional, tag = "11")]
    layer_two: Option<UidLayerTwo>,
}

#[derive(Clone, PartialEq, Message)]
struct UidLayerTwo {
    #[prost(string, tag = "1")]
    uid: String,
}

/// Expected local bindings for one post-QR credential response.
#[derive(Clone, Copy)]
pub struct CredentialResponseContext<'a> {
    /// Confirmed numeric QQ account identifier.
    pub uin: u32,
    /// Fresh key agreement used by the matching credential request.
    pub key_agreement: &'a dyn QqKeyAgreement,
    /// Temporary TGTGT key returned by QR confirmation.
    pub tgtgt_key: &'a [u8],
}

/// Decrypts and validates one post-QR credential response.
///
/// # Errors
///
/// Returns an error for a mismatched account, malformed packet, incomplete success response or
/// failed decryption. A valid QQ rejection is returned as a typed outcome rather than an error.
pub fn decode_credential_exchange_response(
    payload: &[u8],
    context: CredentialResponseContext<'_>,
) -> Result<CredentialExchangeOutcome, CredentialExchangeError> {
    if payload.len() > MAX_LOGIN_PACKET_LEN || context.uin == 0 {
        return Err(CredentialExchangeError::InvalidField);
    }
    let tgtgt_key = key_from_slice(context.tgtgt_key)?;
    let body = decrypt_response_body(payload, context)?;
    let mut reader = WireReader::new(&body);
    if reader.read_u16()? != INTERNAL_COMMAND {
        return Err(CredentialExchangeError::InvalidField);
    }
    let state = reader.read_u8()?;
    let outcome = if state == 0 {
        decode_success(&mut reader, &tgtgt_key)
    } else {
        decode_rejection(&mut reader, state)
    }?;
    reader.finish()?;
    Ok(outcome)
}

fn decrypt_response_body(
    payload: &[u8],
    context: CredentialResponseContext<'_>,
) -> Result<Vec<u8>, CredentialExchangeError> {
    let mut reader = WireReader::new(payload);
    if reader.read_u8()? != 2
        || usize::from(reader.read_u16()?) != payload.len()
        || reader.read_u16()? != WTLOGIN_VERSION
        || reader.read_u16()? != WTLOGIN_COMMAND
    {
        return Err(CredentialExchangeError::InvalidField);
    }
    let _sequence = reader.read_u16()?;
    if reader.read_u32()? != context.uin {
        return Err(CredentialExchangeError::InvalidField);
    }
    let _flag = reader.read_u8()?;
    let _retry = reader.read_u16()?;
    let encrypted_len = reader
        .remaining()
        .checked_sub(1)
        .ok_or(CredentialExchangeError::InvalidField)?;
    let encrypted = reader.read_bytes(encrypted_len)?;
    if reader.read_u8()? != 3 {
        return Err(CredentialExchangeError::InvalidField);
    }
    reader.finish()?;
    decrypt_qq_tea(encrypted, context.key_agreement.tea_key()).map_err(Into::into)
}

fn decode_success(
    reader: &mut WireReader<'_>,
    tgtgt_key: &QqTeaKey,
) -> Result<CredentialExchangeOutcome, CredentialExchangeError> {
    let outer = read_tlv_collection(reader)?;
    let encrypted = required_tlv(&outer, 0x119)?;
    let nested = decrypt_qq_tea(encrypted, tgtgt_key)?;
    let mut nested_reader = WireReader::new(&nested);
    let nested = read_tlv_collection(&mut nested_reader)?;
    nested_reader.finish()?;

    let (age, gender, nickname) = decode_profile(required_tlv(&nested, 0x11a)?)?;
    let uid = decode_uid(required_tlv(&nested, 0x543)?)?;
    let d2_key = required_nonempty(&nested, 0x305)?;
    if d2_key.len() != QqTeaKey::LENGTH {
        return Err(CredentialExchangeError::InvalidField);
    }
    let secrets = CredentialSessionSecrets::new(
        d2_key,
        required_nonempty(&nested, 0x10a)?,
        required_nonempty(&nested, 0x143)?,
        required_nonempty(&nested, 0x106)?,
    );
    Ok(CredentialExchangeOutcome::Success(CredentialLogin::new(
        uid, nickname, age, gender, secrets,
    )))
}

fn decode_rejection(
    reader: &mut WireReader<'_>,
    state: u8,
) -> Result<CredentialExchangeOutcome, CredentialExchangeError> {
    if reader.remaining() == 0 {
        return Ok(CredentialExchangeOutcome::Rejected(
            CredentialRejection::new(state, None, None),
        ));
    }
    let tlvs = read_tlv_collection(reader)?;
    let (tag, message) = tlvs
        .get(&0x146)
        .map(|body| decode_notice(body))
        .transpose()?
        .unwrap_or((None, None));
    Ok(CredentialExchangeOutcome::Rejected(
        CredentialRejection::new(state, tag, message),
    ))
}

fn read_tlv_collection<'a>(
    reader: &mut WireReader<'a>,
) -> Result<BTreeMap<u16, &'a [u8]>, CredentialExchangeError> {
    let count = usize::from(reader.read_u16()?);
    if count == 0 || count > MAX_TLV_COUNT {
        return Err(CredentialExchangeError::InvalidField);
    }
    let mut values = BTreeMap::new();
    for _index in 0..count {
        let tag = reader.read_u16()?;
        let body = reader.read_prefixed_bytes(LengthPrefix::U16Payload, MAX_TLV_LEN)?;
        if values.insert(tag, body).is_some() {
            return Err(CredentialExchangeError::InvalidField);
        }
    }
    Ok(values)
}

fn decode_profile(body: &[u8]) -> Result<(u8, u8, String), CredentialExchangeError> {
    let mut reader = WireReader::new(body);
    let _face = reader.read_u16()?;
    let age = reader.read_u8()?;
    let gender = reader.read_u8()?;
    let nickname = reader.read_prefixed_bytes(LengthPrefix::U8Payload, u8::MAX.into())?;
    reader.finish()?;
    Ok((age, gender, decode_text(nickname)?))
}

fn decode_uid(body: &[u8]) -> Result<String, CredentialExchangeError> {
    if body.len() > MAX_UID_PROTO_LEN {
        return Err(CredentialExchangeError::InvalidField);
    }
    let uid = UidEnvelope::decode(body)
        .map_err(|_error| CredentialExchangeError::InvalidField)?
        .layer_one
        .and_then(|layer| layer.layer_two)
        .map(|layer| layer.uid)
        .filter(|uid| !uid.is_empty() && uid.len() <= MAX_NOTICE_LEN)
        .ok_or(CredentialExchangeError::InvalidField)?;
    Ok(uid)
}

fn decode_notice(body: &[u8]) -> Result<(Option<String>, Option<String>), CredentialExchangeError> {
    let mut reader = WireReader::new(body);
    let _state = reader.read_u32()?;
    let tag = reader.read_prefixed_bytes(LengthPrefix::U16Payload, MAX_NOTICE_LEN)?;
    let message = reader.read_prefixed_bytes(LengthPrefix::U16Payload, MAX_NOTICE_LEN)?;
    let _reserved = reader.read_u32()?;
    reader.finish()?;
    Ok((optional_text(tag)?, optional_text(message)?))
}

fn optional_text(bytes: &[u8]) -> Result<Option<String>, CredentialExchangeError> {
    if bytes.is_empty() {
        Ok(None)
    } else {
        decode_text(bytes).map(Some)
    }
}

fn decode_text(bytes: &[u8]) -> Result<String, CredentialExchangeError> {
    String::from_utf8(bytes.to_vec()).map_err(|_error| CredentialExchangeError::InvalidField)
}

fn required_tlv<'a>(
    values: &BTreeMap<u16, &'a [u8]>,
    tag: u16,
) -> Result<&'a [u8], CredentialExchangeError> {
    values
        .get(&tag)
        .copied()
        .ok_or(CredentialExchangeError::InvalidField)
}

fn required_nonempty<'a>(
    values: &BTreeMap<u16, &'a [u8]>,
    tag: u16,
) -> Result<&'a [u8], CredentialExchangeError> {
    required_tlv(values, tag).and_then(|body| {
        if body.is_empty() {
            Err(CredentialExchangeError::InvalidField)
        } else {
            Ok(body)
        }
    })
}

fn key_from_slice(bytes: &[u8]) -> Result<QqTeaKey, CredentialExchangeError> {
    let mut key = [0_u8; QqTeaKey::LENGTH];
    if bytes.len() != key.len() {
        return Err(CredentialExchangeError::InvalidField);
    }
    key.copy_from_slice(bytes);
    Ok(QqTeaKey::new(key))
}
