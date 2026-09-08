use std::io;

use qq_domain::{DevicePortrait, DevicePower, DeviceProfile};
use serde::{Deserialize, Serialize};

use super::invalid;

const DEVICE_SCHEMA_VERSION: u16 = 4;

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct DeviceFile {
    schema_version: u16,
    guid: String,
    mac_address: String,
    archetype: String,
    profile_category: String,
    hostname: String,
    device_name: String,
    hardware_model: String,
    os_release: String,
    distro: String,
    desktop_environment: String,
    session_type: String,
    login_name: String,
    network_interface: String,
    root_filesystem_bytes: String,
    memory_total_kib: String,
    timezone: String,
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
        let portrait = DevicePortrait::new(
            self.archetype,
            self.profile_category,
            self.hostname,
            self.device_name,
            self.hardware_model,
            self.os_release,
            self.distro,
            self.desktop_environment,
            self.session_type,
            self.login_name,
            self.network_interface,
            self.root_filesystem_bytes,
            self.memory_total_kib,
            self.timezone,
        )
        .map_err(|_error| invalid())?;
        DeviceProfile::from_portrait(
            parse_guid(&self.guid)?,
            parse_mac(&self.mac_address)?,
            portrait,
            self.power.into(),
        )
        .map_err(|_error| invalid())
    }

    fn from_profile(profile: &DeviceProfile) -> Self {
        let portrait = profile.portrait();
        Self {
            schema_version: DEVICE_SCHEMA_VERSION,
            guid: format_guid(profile.guid()),
            mac_address: format_mac(*profile.mac_address()),
            archetype: portrait.archetype().to_owned(),
            profile_category: portrait.profile_category().to_owned(),
            hostname: portrait.hostname().to_owned(),
            device_name: portrait.device_name().to_owned(),
            hardware_model: portrait.hardware_model().to_owned(),
            os_release: portrait.os_release().to_owned(),
            distro: portrait.distro().to_owned(),
            desktop_environment: portrait.desktop_environment().to_owned(),
            session_type: portrait.session_type().to_owned(),
            login_name: portrait.login_name().to_owned(),
            network_interface: portrait.network_interface().to_owned(),
            root_filesystem_bytes: portrait.root_filesystem_bytes().to_owned(),
            memory_total_kib: portrait.memory_total_kib().to_owned(),
            timezone: portrait.timezone().to_owned(),
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
    let canonical = parse_hex::<16>(value.bytes().filter(|byte| *byte != b'-'))?;
    Ok(canonical_to_wire_guid(canonical))
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
    if hexadecimal.len() != N * 2 {
        return Err(invalid());
    }
    let mut parsed = [0_u8; N];
    for (target, pair) in parsed.iter_mut().zip(hexadecimal.chunks_exact(2)) {
        let text = core::str::from_utf8(pair).map_err(|_error| invalid())?;
        *target = u8::from_str_radix(text, 16).map_err(|_error| invalid())?;
    }
    Ok(parsed)
}

pub(super) fn format_guid(value: &[u8; 16]) -> String {
    let value = wire_to_canonical_guid(*value);
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

pub(super) fn canonical_to_wire_guid(mut value: [u8; 16]) -> [u8; 16] {
    value[..4].reverse();
    value[4..6].reverse();
    value[6..8].reverse();
    value
}

fn wire_to_canonical_guid(value: [u8; 16]) -> [u8; 16] {
    canonical_to_wire_guid(value)
}

fn format_mac(value: [u8; 6]) -> String {
    format!(
        "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
        value[0], value[1], value[2], value[3], value[4], value[5]
    )
}
