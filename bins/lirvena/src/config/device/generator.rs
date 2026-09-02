use std::io;

use qq_domain::{DevicePower, DeviceProfile};

use super::invalid;

#[derive(Clone, Copy)]
struct SyntheticTemplate {
    model: &'static str,
    kernel_version: &'static str,
    portable: bool,
}

const TEMPLATES: &[SyntheticTemplate] = &[
    SyntheticTemplate {
        model: "Synthetic desktop",
        kernel_version: "6.8.0-generic",
        portable: false,
    },
    SyntheticTemplate {
        model: "Synthetic compact desktop",
        kernel_version: "6.6.0-amd64",
        portable: false,
    },
    SyntheticTemplate {
        model: "Synthetic portable",
        kernel_version: "6.8.0-generic",
        portable: true,
    },
];

pub(super) fn generate() -> Result<DeviceProfile, io::Error> {
    let mut random = [0_u8; 24];
    getrandom::fill(&mut random).map_err(|_error| io::Error::other("device generation failed"))?;
    let template = TEMPLATES[usize::from(random[22]) * TEMPLATES.len() / 256];
    let mut guid = <[u8; 16]>::try_from(&random[..16]).map_err(|_error| invalid())?;
    guid[6] = (guid[6] & 0x0f) | 0x40;
    guid[8] = (guid[8] & 0x3f) | 0x80;
    let mut mac_address = <[u8; 6]>::try_from(&random[16..22]).map_err(|_error| invalid())?;
    mac_address[0] = (mac_address[0] | 0x02) & 0xfe;
    let power = if template.portable {
        DevicePower::Portable {
            percent: 85,
            charging: true,
        }
    } else {
        DevicePower::Desktop
    };
    DeviceProfile::new(
        guid,
        mac_address,
        "Lirvena device".to_owned(),
        template.model.to_owned(),
        "Linux".to_owned(),
        template.kernel_version.to_owned(),
        power,
    )
    .map_err(|_error| invalid())
}
