use qq_envelope::QqTeaKey;
use qq_profile::LinuxNtProfile;
use qq_wire::{LengthPrefix, WireError, WireWriter};

const VERSION: u16 = 8_001;
const EXTENSION_VERSION: u8 = 3;
const COMMAND_VERSION: u8 = 135;
const ECDH_PUBLIC_ID: u8 = 19;
const ECDH_TYPE: u16 = 0x102;
const MAX_PACKET_LEN: usize = 64 * 1024;

pub(crate) const TRANS_EMP_COMMAND: u16 = 2_066;

#[derive(Clone, Copy)]
pub(crate) struct WtLoginPacket<'a> {
    pub(crate) profile: &'a LinuxNtProfile,
    pub(crate) command: u16,
    pub(crate) sequence: u16,
    pub(crate) uin: u32,
    pub(crate) random_key: &'a QqTeaKey,
    pub(crate) public_key: &'a [u8],
    pub(crate) encrypted: &'a [u8],
}

pub(crate) fn encode(parts: WtLoginPacket<'_>) -> Result<Vec<u8>, WireError> {
    let mut body = WireWriter::new(MAX_PACKET_LEN);
    body.put_u16(VERSION)?;
    body.put_u16(parts.command)?;
    body.put_u16(parts.sequence)?;
    body.put_u32(parts.uin)?;
    body.put_u8(EXTENSION_VERSION)?;
    body.put_u8(COMMAND_VERSION)?;
    body.put_u32(0)?;
    body.put_u8(ECDH_PUBLIC_ID)?;
    body.put_u16(0)?;
    body.put_u16(parts.profile.app_client_version())?;
    body.put_u32(0)?;
    body.put_u8(1)?;
    body.put_u8(1)?;
    body.put_bytes(parts.random_key.as_bytes())?;
    body.put_u16(ECDH_TYPE)?;
    body.put_prefixed_bytes(LengthPrefix::U16Payload, parts.public_key)?;
    body.put_bytes(parts.encrypted)?;
    body.put_u8(3)?;
    let body = body.finish();
    let declared_len = body
        .len()
        .checked_add(3)
        .and_then(|length| u16::try_from(length).ok())
        .ok_or(WireError::LengthOverflow)?;

    let mut output = WireWriter::new(MAX_PACKET_LEN);
    output.put_u8(2)?;
    output.put_u16(declared_len)?;
    output.put_bytes(&body)?;
    Ok(output.finish())
}
