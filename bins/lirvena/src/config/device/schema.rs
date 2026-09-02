use std::io;

use qq_domain::{DevicePower, DeviceProfile};
use serde::{Deserialize, Serialize};

use super::invalid;

const DEVICE_SCHEMA_VERSION: u16 = 1;

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct DeviceFile {
    schema_version: u16,
    guid: String,
    mac_address: String,
    name: String,
    model: String,
    system_kernel: String,
    kernel_version: String,
    power: PowerFile,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case", tag = "kind")]
enum PowerFile {
    Desktop,
    Portable { percent: u8, charging: bool },
}

pub(super) fn decode(bytes: &[u8]) -> Result<DeviceProfile, io::Error> {
    let file: DeviceFile = serde_json::from_slice(bytes).map_err(|_error| invalid())?;
    file.into_profile()
}

pub(super) fn encode(profile: &DeviceProfile) -> Result<Vec<u8>, io::Error> {
    let mut encoded = serde_json::to_vec_pretty(&DeviceFile::from_profile(profile))
        .map_err(|_error| invalid())?;
    encoded.push(b'\n');
    Ok(encoded)
}

impl DeviceFile {
    fn into_profile(self) -> Result<DeviceProfile, io::Error> {
        if self.schema_version != DEVICE_SCHEMA_VERSION {
            return Err(invalid());
        }
        DeviceProfile::new(
            parse_guid(&self.guid)?,
            parse_mac(&self.mac_address)?,
            self.name,
            self.model,
            self.system_kernel,
            self.kernel_version,
            self.power.into(),
        )
        .map_err(|_error| invalid())
    }

    fn from_profile(profile: &DeviceProfile) -> Self {
        Self {
            schema_version: DEVICE_SCHEMA_VERSION,
            guid: format_guid(profile.guid()),
            mac_address: format_mac(*profile.mac_address()),
            name: profile.name().to_owned(),
            model: profile.model().to_owned(),
            system_kernel: profile.system_kernel().to_owned(),
            kernel_version: profile.kernel_version().to_owned(),
            power: profile.power().into(),
        }
    }
}

impl From<PowerFile> for DevicePower {
    fn from(value: PowerFile) -> Self {
        match value {
            PowerFile::Desktop => Self::Desktop,
            PowerFile::Portable { percent, charging } => Self::Portable { percent, charging },
        }
    }
}

impl From<DevicePower> for PowerFile {
    fn from(value: DevicePower) -> Self {
        match value {
            DevicePower::Desktop => Self::Desktop,
            DevicePower::Portable { percent, charging } => Self::Portable { percent, charging },
        }
    }
}

fn parse_guid(value: &str) -> Result<[u8; 16], io::Error> {
    if value.len() != 36
        || ![8, 13, 18, 23]
            .into_iter()
            .all(|index| value.as_bytes()[index] == b'-')
    {
        return Err(invalid());
    }
    parse_hex::<16>(value.bytes().filter(|byte| *byte != b'-'))
}

fn parse_mac(value: &str) -> Result<[u8; 6], io::Error> {
    if value.len() != 17
        || ![2, 5, 8, 11, 14]
            .into_iter()
            .all(|index| value.as_bytes()[index] == b':')
    {
        return Err(invalid());
    }
    parse_hex::<6>(value.bytes().filter(|byte| *byte != b':'))
}

fn parse_hex<const N: usize>(bytes: impl Iterator<Item = u8>) -> Result<[u8; N], io::Error> {
    let hexadecimal = bytes.collect::<Vec<_>>();
    let mut parsed = [0_u8; N];
    for (target, pair) in parsed.iter_mut().zip(hexadecimal.chunks_exact(2)) {
        let text = core::str::from_utf8(pair).map_err(|_error| invalid())?;
        *target = u8::from_str_radix(text, 16).map_err(|_error| invalid())?;
    }
    Ok(parsed)
}

pub(super) fn format_guid(value: &[u8; 16]) -> String {
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        value[0],
        value[1],
        value[2],
        value[3],
        value[4],
        value[5],
        value[6],
        value[7],
        value[8],
        value[9],
        value[10],
        value[11],
        value[12],
        value[13],
        value[14],
        value[15]
    )
}

fn format_mac(value: [u8; 6]) -> String {
    format!(
        "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
        value[0], value[1], value[2], value[3], value[4], value[5]
    )
}
