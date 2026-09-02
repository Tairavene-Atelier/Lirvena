use qq_envelope::{QqTeaKey, encrypt_qq_tea};
use qq_profile::LinuxNtProfile;
use qq_wire::{LengthPrefix, WireWriter};

use crate::{CredentialExchangeError, QrDevice, QrLoginSecrets};

const MAX_LOGIN_PACKET_LEN: usize = 64 * 1024;
const LOGIN_TLV_COUNT: u16 = 15;

pub(super) fn build_login_tlvs(
    profile: &LinuxNtProfile,
    device: &QrDevice,
    secrets: &QrLoginSecrets,
    uin: u32,
) -> Result<Vec<u8>, CredentialExchangeError> {
    let mut output = WireWriter::new(MAX_LOGIN_PACKET_LEN);
    output.put_u16(LOGIN_TLV_COUNT)?;
    write_tlv(&mut output, 0x106, secrets.temporary_password())?;
    write_tlv(
        &mut output,
        0x144,
        &encrypted_device_bundle(profile, device, secrets)?,
    )?;
    write_tlv(&mut output, 0x116, &capability_body(profile)?)?;
    write_tlv(&mut output, 0x142, &package_body(profile)?)?;
    write_tlv(&mut output, 0x145, device.guid())?;
    write_tlv(&mut output, 0x018, &account_body(uin)?)?;
    write_tlv(&mut output, 0x141, &network_body()?)?;
    write_tlv(&mut output, 0x177, &sdk_body(profile)?)?;
    write_tlv(&mut output, 0x191, &[0])?;
    write_tlv(&mut output, 0x100, &application_body(profile)?)?;
    write_tlv(&mut output, 0x107, &[0, 1, 0x0d, 0, 0, 1])?;
    write_tlv(&mut output, 0x318, &[])?;
    write_tlv(&mut output, 0x16a, secrets.no_picture_signature())?;
    write_tlv(&mut output, 0x166, &[5])?;
    write_tlv(&mut output, 0x521, &product_body()?)?;
    Ok(output.finish())
}

fn encrypted_device_bundle(
    profile: &LinuxNtProfile,
    device: &QrDevice,
    secrets: &QrLoginSecrets,
) -> Result<Vec<u8>, CredentialExchangeError> {
    let key_bytes: [u8; QqTeaKey::LENGTH] = secrets
        .tgtgt_key()
        .try_into()
        .map_err(|_| CredentialExchangeError::InvalidField)?;
    let mut bundle = WireWriter::new(MAX_LOGIN_PACKET_LEN);
    bundle.put_u16(4)?;
    write_tlv(&mut bundle, 0x16e, device.name().as_bytes())?;
    write_tlv(&mut bundle, 0x147, &identity_body(profile)?)?;
    write_tlv(&mut bundle, 0x128, &device_body(profile, device)?)?;
    write_tlv(&mut bundle, 0x124, &[0; 12])?;
    encrypt_qq_tea(&bundle.finish(), &QqTeaKey::new(key_bytes)).map_err(Into::into)
}

fn capability_body(profile: &LinuxNtProfile) -> Result<Vec<u8>, CredentialExchangeError> {
    let mut body = WireWriter::new(16);
    body.put_u8(0)?;
    body.put_u32(profile.login_misc_bitmap())?;
    body.put_u32(profile.sub_sig_map())?;
    body.put_u8(0)?;
    Ok(body.finish())
}

fn package_body(profile: &LinuxNtProfile) -> Result<Vec<u8>, CredentialExchangeError> {
    let mut body = WireWriter::new(128);
    body.put_u16(0)?;
    body.put_prefixed_bytes(LengthPrefix::U16Payload, profile.package_name().as_bytes())?;
    Ok(body.finish())
}

fn account_body(uin: u32) -> Result<Vec<u8>, CredentialExchangeError> {
    let mut body = WireWriter::new(32);
    body.put_u16(0)?;
    body.put_u32(5)?;
    body.put_u32(0)?;
    body.put_u32(8_001)?;
    body.put_u32(uin)?;
    body.put_u16(0)?;
    body.put_u16(0)?;
    Ok(body.finish())
}

fn network_body() -> Result<Vec<u8>, CredentialExchangeError> {
    let mut body = WireWriter::new(32);
    body.put_u16(0)?;
    body.put_prefixed_bytes(LengthPrefix::U16Payload, b"Unknown")?;
    body.put_u16(0)?;
    body.put_prefixed_bytes(LengthPrefix::U16Payload, &[])?;
    Ok(body.finish())
}

fn sdk_body(profile: &LinuxNtProfile) -> Result<Vec<u8>, CredentialExchangeError> {
    let mut body = WireWriter::new(96);
    body.put_u8(1)?;
    body.put_u32(0)?;
    body.put_prefixed_bytes(LengthPrefix::U16Payload, profile.login_sdk().as_bytes())?;
    Ok(body.finish())
}

fn application_body(profile: &LinuxNtProfile) -> Result<Vec<u8>, CredentialExchangeError> {
    let mut body = WireWriter::new(32);
    body.put_u16(0)?;
    body.put_u32(5)?;
    body.put_u32(profile.app_id())?;
    body.put_u32(profile.sub_app_id())?;
    body.put_u32(u32::from(profile.app_client_version()))?;
    body.put_u32(profile.main_sig_map())?;
    Ok(body.finish())
}

fn product_body() -> Result<Vec<u8>, CredentialExchangeError> {
    let mut body = WireWriter::new(32);
    body.put_u32(0x13)?;
    body.put_prefixed_bytes(LengthPrefix::U16Payload, b"basicim")?;
    Ok(body.finish())
}

fn identity_body(profile: &LinuxNtProfile) -> Result<Vec<u8>, CredentialExchangeError> {
    let mut body = WireWriter::new(256);
    body.put_u32(profile.app_id())?;
    body.put_prefixed_bytes(LengthPrefix::U16Payload, profile.pt_version().as_bytes())?;
    body.put_prefixed_bytes(LengthPrefix::U16Payload, profile.package_name().as_bytes())?;
    Ok(body.finish())
}

fn device_body(
    profile: &LinuxNtProfile,
    device: &QrDevice,
) -> Result<Vec<u8>, CredentialExchangeError> {
    let mut body = WireWriter::new(256);
    body.put_u16(0)?;
    body.put_u8(0)?;
    body.put_u8(0)?;
    body.put_u8(0)?;
    body.put_u32(0)?;
    body.put_prefixed_bytes(
        LengthPrefix::U16Payload,
        profile.operating_system().as_bytes(),
    )?;
    body.put_prefixed_bytes(LengthPrefix::U16Payload, device.guid())?;
    body.put_u16(0)?;
    Ok(body.finish())
}

fn write_tlv(
    writer: &mut WireWriter,
    tag: u16,
    body: &[u8],
) -> Result<(), CredentialExchangeError> {
    writer.put_u16(tag)?;
    writer.put_prefixed_bytes(LengthPrefix::U16Payload, body)?;
    Ok(())
}
