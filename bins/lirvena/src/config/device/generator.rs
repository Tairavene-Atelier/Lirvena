use std::io;

use qq_domain::{DevicePower, DeviceProfile};

use super::{invalid, schema::canonical_to_wire_guid};

#[derive(Clone, Copy)]
struct SyntheticTemplate {
    model: &'static str,
    kernel: &'static str,
    portable: bool,
    weight: u16,
}

const TEMPLATES: &[SyntheticTemplate] = &[
    template("ThinkCentre M720q", "5.10.0-amd64-desktop", false, 18),
    template("OptiPlex 3070 Micro", "6.6.25-amd64-desktop-hwe", false, 18),
    template("ProDesk 400 G5 DM", "5.15.0-amd64-desktop", false, 16),
    template("ThinkCentre M75q Gen 2", "5.10.0-amd64-desktop", false, 16),
    template("OptiPlex 7090", "6.6.25-amd64-desktop-hwe", false, 14),
    template("Steam Deck", "6.5.0-valve22-1-neptune-65", true, 12),
    template("ROG Ally RC71L", "6.8.10-bazzite", true, 10),
    template("Legion Go 8APU1", "6.8.10-bazzite", true, 9),
    template("Nitro AN515-58", "6.8.10-bazzite", false, 8),
    template("Legion 5 15ACH6", "6.8.10-bazzite", false, 8),
    template("ROG Zephyrus G14 GA401", "6.8.10-bazzite", false, 7),
    template("Framework Laptop 13", "6.8.9-300.fc40.x86_64", true, 8),
    template("System76 Lemur Pro", "6.8.0-31-generic", false, 7),
    template("TUXEDO Pulse 14", "6.8.0-31-generic", false, 7),
    template("Slimbook Executive 14", "6.8.9-300.fc40.x86_64", true, 6),
    template("ThinkPad T14 Gen 3", "6.8.0-60-generic", true, 6),
    template("ThinkPad X1 Carbon Gen 10", "6.6.15-amd64", true, 4),
    template("XPS 13 9310", "6.1.0-21-amd64", true, 4),
    template("MateBook 14 2021", "6.9.7-arch1-1", true, 4),
    template("ThinkPad T480", "6.1.0-21-amd64", true, 4),
    template("Latitude 5420", "6.8.0-31-generic", true, 4),
    template("EliteBook 845 G8", "6.5.0-35-generic", true, 4),
    template("ThinkPad X260", "6.1.0-21-amd64", true, 4),
    template("OptiPlex 7040", "6.1.0-21-amd64", false, 4),
    template("H110M-S2PH", "6.6.15-amd64", false, 3),
    template("X99-A II", "6.1.0-21-amd64", false, 3),
    template("X99-UD4", "6.6.15-amd64", false, 3),
    template("B450M MORTAR MAX", "6.8.0-31-generic", false, 3),
    template("B550M AORUS ELITE", "6.8.9-300.fc40.x86_64", false, 3),
    template("SER5 MAX", "6.7.12-200.fc39.x86_64", false, 4),
    template("B660M DS3H DDR4", "6.7.12-200.fc39.x86_64", false, 3),
    template("ProArt X870E-CREATOR WIFI", "6.10.2-arch1-1", false, 3),
    template("ROG MAXIMUS Z790 HERO", "6.9.12-200.fc40.x86_64", false, 3),
    template("Z790 AORUS MASTER X", "6.9.12-200.fc40.x86_64", false, 2),
    template("MPG Z890 CARBON WIFI", "6.11.4-arch1-1", false, 2),
    template("Pro WS WRX90E-SAGE SE", "6.10.2-arch1-1", false, 2),
    template("Precision 5860 Tower", "6.8.0-31-generic", false, 2),
    template("ThinkStation P3 Tower", "6.8.0-31-generic", false, 2),
];

const fn template(
    model: &'static str,
    kernel: &'static str,
    portable: bool,
    weight: u16,
) -> SyntheticTemplate {
    SyntheticTemplate {
        model,
        kernel,
        portable,
        weight,
    }
}

pub(super) fn generate() -> Result<DeviceProfile, io::Error> {
    let mut random = [0_u8; 24];
    getrandom::fill(&mut random).map_err(|_error| io::Error::other("device generation failed"))?;
    let template = select_template(u16::from_be_bytes([random[22], random[23]]));
    let mut canonical_guid = <[u8; 16]>::try_from(&random[..16]).map_err(|_error| invalid())?;
    canonical_guid[6] = (canonical_guid[6] & 0x0f) | 0x40;
    canonical_guid[8] = (canonical_guid[8] & 0x3f) | 0x80;
    let guid = canonical_to_wire_guid(canonical_guid);
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
        template.model.to_owned(),
        template.model.to_owned(),
        format!("Linux {}", template.kernel),
        template.kernel.to_owned(),
        power,
    )
    .map_err(|_error| invalid())
}

fn select_template(sample: u16) -> SyntheticTemplate {
    let total = TEMPLATES
        .iter()
        .map(|template| template.weight)
        .sum::<u16>();
    let mut selected = sample % total;
    for template in TEMPLATES {
        if selected < template.weight {
            return *template;
        }
        selected -= template.weight;
    }
    TEMPLATES[TEMPLATES.len() - 1]
}
