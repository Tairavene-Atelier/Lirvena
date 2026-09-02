use prost::Message;
use qq_profile::LinuxNtProfile;
use qq_wire::{LengthPrefix, WireWriter};

use crate::{QrDevice, QrPacketError};

const MAX_TLV_BODY_LEN: usize = 8 * 1024;

#[derive(Clone, PartialEq, Message)]
struct NtOperatingSystem {
    #[prost(string, tag = "1")]
    operating_system: String,
    #[prost(string, tag = "2")]
    device_name: String,
}

#[derive(Clone, PartialEq, Message)]
struct QrRequestInfo {
    #[prost(message, optional, tag = "1")]
    system: Option<NtOperatingSystem>,
    #[prost(bytes = "vec", tag = "4")]
    kind: Vec<u8>,
}

pub(super) fn build_fetch_tlvs(
    profile: &LinuxNtProfile,
    device: &QrDevice,
) -> Result<Vec<u8>, QrPacketError> {
    let bodies = [
        (0x016, tlv_16(profile, device)?),
        (0x01b, tlv_1b()?),
        (0x01d, tlv_1d(profile)?),
        (0x033, device.guid().to_vec()),
        (0x035, profile.sso_version().to_be_bytes().to_vec()),
        (0x066, profile.sso_version().to_be_bytes().to_vec()),
        (0x0d1, tlv_d1(profile, device)),
    ];
    let mut output = WireWriter::new(MAX_TLV_BODY_LEN);
    output.put_u16(u16::try_from(bodies.len()).map_err(|_error| QrPacketError::InvalidField)?)?;
    for (tag, body) in bodies {
        output.put_u16(tag)?;
        output.put_prefixed_bytes(LengthPrefix::U16Payload, &body)?;
    }
    Ok(output.finish())
}

fn tlv_16(profile: &LinuxNtProfile, device: &QrDevice) -> Result<Vec<u8>, QrPacketError> {
    let mut output = WireWriter::new(MAX_TLV_BODY_LEN);
    output.put_u32(0)?;
    output.put_u32(profile.app_id())?;
    output.put_u32(profile.qr_app_id())?;
    output.put_bytes(device.guid())?;
    output.put_prefixed_bytes(LengthPrefix::U16Payload, profile.package_name().as_bytes())?;
    output.put_prefixed_bytes(LengthPrefix::U16Payload, profile.pt_version().as_bytes())?;
    output.put_prefixed_bytes(LengthPrefix::U16Payload, profile.package_name().as_bytes())?;
    Ok(output.finish())
}

fn tlv_1b() -> Result<Vec<u8>, QrPacketError> {
    let mut output = WireWriter::new(MAX_TLV_BODY_LEN);
    for value in [0, 0, 3, 4, 72, 2, 2] {
        output.put_u32(value)?;
    }
    output.put_u16(0)?;
    Ok(output.finish())
}

fn tlv_1d(profile: &LinuxNtProfile) -> Result<Vec<u8>, QrPacketError> {
    let mut output = WireWriter::new(MAX_TLV_BODY_LEN);
    output.put_u8(1)?;
    output.put_u32(profile.misc_bitmap())?;
    output.put_u32(0)?;
    output.put_u8(0)?;
    Ok(output.finish())
}

fn tlv_d1(profile: &LinuxNtProfile, device: &QrDevice) -> Vec<u8> {
    QrRequestInfo {
        system: Some(NtOperatingSystem {
            operating_system: profile.operating_system().to_owned(),
            device_name: device.name().to_owned(),
        }),
        kind: vec![0x30, 0x01],
    }
    .encode_to_vec()
}
