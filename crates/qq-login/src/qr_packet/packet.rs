use qq_envelope::QqTeaKey;
use qq_profile::LinuxNtProfile;
use qq_wire::{LengthPrefix, WireWriter};

use crate::QrPacketError;

const WTLOGIN_COMMAND: u16 = 2_066;
const MAX_LOGIN_PACKET_LEN: usize = 64 * 1024;

pub(super) fn build_transaction(command: u16, body: &[u8]) -> Result<Vec<u8>, QrPacketError> {
    let declared_len = body
        .len()
        .checked_add(44)
        .and_then(|length| u16::try_from(length).ok())
        .ok_or(QrPacketError::InvalidField)?;
    let mut transaction = WireWriter::new(MAX_LOGIN_PACKET_LEN);
    transaction.put_u8(2)?;
    transaction.put_u16(declared_len)?;
    transaction.put_u16(command)?;
    transaction.put_bytes(&[0; 21])?;
    transaction.put_u8(3)?;
    transaction.put_u16(0)?;
    transaction.put_u16(0x32)?;
    transaction.put_u32(0)?;
    transaction.put_u64(0)?;
    transaction.put_bytes(body)?;
    transaction.put_u8(3)?;
    Ok(transaction.finish())
}

pub(super) fn build_request_data(
    profile: &LinuxNtProfile,
    unix_seconds: u32,
    transaction: &[u8],
) -> Result<Vec<u8>, QrPacketError> {
    let mut request_body = WireWriter::new(MAX_LOGIN_PACKET_LEN);
    request_body.put_u32(unix_seconds)?;
    request_body.put_bytes(transaction)?;
    let request_body = request_body.finish();
    let request_len =
        u16::try_from(request_body.len()).map_err(|_error| QrPacketError::InvalidField)?;

    let mut data = WireWriter::new(MAX_LOGIN_PACKET_LEN);
    data.put_u8(0)?;
    data.put_u16(request_len)?;
    data.put_u32(profile.app_id())?;
    data.put_u32(0x72)?;
    data.put_prefixed_bytes(LengthPrefix::U16Payload, &[])?;
    data.put_prefixed_bytes(LengthPrefix::U8Payload, &[])?;
    data.put_bytes(&request_body)?;
    Ok(data.finish())
}

pub(super) fn build_wtlogin_packet(
    profile: &LinuxNtProfile,
    random_key: &QqTeaKey,
    public_key: &[u8],
    encrypted: &[u8],
) -> Result<Vec<u8>, QrPacketError> {
    let mut body = WireWriter::new(MAX_LOGIN_PACKET_LEN);
    body.put_u16(8_001)?;
    body.put_u16(WTLOGIN_COMMAND)?;
    body.put_u16(0)?;
    body.put_u32(0)?;
    body.put_u8(3)?;
    body.put_u8(135)?;
    body.put_u32(0)?;
    body.put_u8(2)?;
    body.put_u16(0)?;
    body.put_u16(profile.app_client_version())?;
    body.put_u32(0)?;
    body.put_u8(1)?;
    body.put_u8(1)?;
    body.put_bytes(random_key.as_bytes())?;
    body.put_u16(0x102)?;
    body.put_prefixed_bytes(LengthPrefix::U16Payload, public_key)?;
    body.put_bytes(encrypted)?;
    body.put_u8(3)?;
    let body = body.finish();
    let declared_len = body
        .len()
        .checked_add(3)
        .and_then(|length| u16::try_from(length).ok())
        .ok_or(QrPacketError::InvalidField)?;

    let mut output = WireWriter::new(MAX_LOGIN_PACKET_LEN);
    output.put_u8(2)?;
    output.put_u16(declared_len)?;
    output.put_bytes(&body)?;
    Ok(output.finish())
}
