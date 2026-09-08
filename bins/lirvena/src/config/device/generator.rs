use std::io;

use qq_domain::{DevicePortrait, DevicePower, DeviceProfile};

use super::{invalid, schema::canonical_to_wire_guid};

#[derive(Clone, Copy)]
struct SyntheticTemplate {
    archetype: &'static str,
    category: &'static str,
    host_prefix: &'static str,
    device_name: &'static str,
    hardware_model: &'static str,
    os_release: &'static str,
    distro: &'static str,
    desktop_environment: &'static str,
    session_type: &'static str,
    login_name: &'static str,
    network_interface: &'static str,
    root_filesystem_bytes: &'static str,
    memory_total_kib: &'static str,
    portable: bool,
    weight: u16,
}

const TEMPLATES: &[SyntheticTemplate] = &[
    template(
        "uos-m720q",
        "domestic-enterprise",
        "uos-office",
        "ThinkCentre M720q",
        "Lenovo ThinkCentre M720q",
        "Linux 5.10.0-amd64-desktop x86_64",
        "UOS Desktop 20",
        "DDE",
        "x11",
        "user",
        "enp0s31f6",
        "499963174912",
        "16325684",
        false,
        16,
    ),
    template(
        "deepin-optiplex3070",
        "domestic-enterprise",
        "deepin-desk",
        "OptiPlex 3070 Micro",
        "Dell OptiPlex 3070 Micro",
        "Linux 6.6.25-amd64-desktop-hwe x86_64",
        "deepin 23",
        "DDE",
        "x11",
        "user",
        "enp1s0",
        "499963174912",
        "16325684",
        false,
        13,
    ),
    template(
        "ubuntu-xps9310-developer",
        "linux-preinstalled",
        "xps-dev",
        "XPS 13 9310",
        "Dell XPS 13 9310",
        "Linux 5.15.0-139-generic x86_64",
        "Ubuntu 20.04.6 LTS",
        "GNOME",
        "x11",
        "developer",
        "wlp0s20f3",
        "255852544000",
        "7864320",
        true,
        10,
    ),
    template(
        "popos-lemurpro11",
        "linux-preinstalled",
        "lemur",
        "Lemur Pro lemp11",
        "System76 Lemur Pro lemp11",
        "Linux 6.9.3-76060903-generic x86_64",
        "Pop!_OS 22.04 LTS",
        "GNOME",
        "x11",
        "sys76",
        "wlp0s20f3",
        "499963174912",
        "16325684",
        true,
        8,
    ),
    template(
        "steamos-jupiter",
        "gaming-linux-native",
        "steamdeck",
        "Steam Deck",
        "Valve Jupiter",
        "Linux 6.5.0-valve22-1-neptune-65 x86_64",
        "SteamOS 3 (Holo)",
        "KDE",
        "x11",
        "deck",
        "wlan0",
        "10737418240",
        "15332852",
        true,
        8,
    ),
    template(
        "bazzite-rog-ally-reimage",
        "gaming-linux-reimage",
        "bazzite",
        "ROG Ally RC71L",
        "ASUSTeK ROG Ally RC71L",
        "Linux 6.8.10-bazzite x86_64",
        "Bazzite 40",
        "KDE",
        "wayland",
        "bazzite",
        "wlan0",
        "499963174912",
        "15324572",
        true,
        4,
    ),
    template(
        "debian-z440-e5v4",
        "legacy-linux-revival",
        "z440",
        "Z440 Workstation",
        "HP Z440 Workstation",
        "Linux 6.1.0-35-amd64 x86_64",
        "Debian GNU/Linux 12",
        "XFCE",
        "x11",
        "work",
        "enp0s25",
        "999653638144",
        "32811340",
        false,
        10,
    ),
    template(
        "mint-optiplex7040-i5",
        "legacy-linux-revival",
        "mint-desk",
        "OptiPlex 7040",
        "Dell OptiPlex 7040",
        "Linux 5.15.0-126-generic x86_64",
        "Linux Mint 21.3",
        "Cinnamon",
        "x11",
        "user",
        "enp0s31f6",
        "255852544000",
        "8032460",
        false,
        9,
    ),
    template(
        "lubuntu-thinkpad-x230",
        "legacy-linux-revival",
        "x230",
        "ThinkPad X230",
        "Lenovo ThinkPad X230",
        "Linux 6.8.0-52-generic x86_64",
        "Lubuntu 24.04 LTS",
        "LXQt",
        "x11",
        "user",
        "wlp3s0",
        "255852544000",
        "8032460",
        true,
        7,
    ),
    template(
        "arch-x670-ryzen9-7950x",
        "high-end-linux-enthusiast",
        "zen4",
        "X670 AORUS ELITE AX",
        "Gigabyte X670 AORUS ELITE AX",
        "Linux 6.9.7-arch1-1 x86_64",
        "Arch Linux",
        "KDE",
        "wayland",
        "aurora",
        "enp6s0",
        "1999307276288",
        "65781248",
        false,
        2,
    ),
    template(
        "popos-thelio-9950x",
        "high-end-linux-enthusiast",
        "thelio",
        "Thelio Mira thelio-r5-n1",
        "System76 Thelio Mira thelio-r5-n1",
        "Linux 6.12.10-76061203-generic x86_64",
        "Pop!_OS 24.04 LTS",
        "COSMIC",
        "wayland",
        "developer",
        "enp5s0",
        "1999307276288",
        "49315840",
        false,
        2,
    ),
    template(
        "arch-z790-i9-13900k",
        "high-end-linux-enthusiast",
        "raptor",
        "PRIME Z790-P WIFI",
        "ASUSTeK PRIME Z790-P WIFI",
        "Linux 6.9.7-arch1-1 x86_64",
        "Arch Linux",
        "KDE",
        "wayland",
        "user",
        "enp4s0",
        "1999307276288",
        "65781248",
        false,
        1,
    ),
    template(
        "fedora-z790-i9-14900k",
        "high-end-linux-enthusiast",
        "raptor14",
        "PRO Z790-A MAX WIFI",
        "MSI PRO Z790-A MAX WIFI",
        "Linux 6.9.7-200.fc40.x86_64 x86_64",
        "Fedora Linux 40",
        "GNOME",
        "wayland",
        "user",
        "enp5s0",
        "1999307276288",
        "65781248",
        false,
        1,
    ),
    template(
        "ubuntu-z890-ultra9-285k",
        "high-end-linux-enthusiast",
        "arrow",
        "PRIME Z890-P WIFI",
        "ASUSTeK PRIME Z890-P WIFI",
        "Linux 6.11.0-17-generic x86_64",
        "Ubuntu 24.04.2 LTS",
        "GNOME",
        "wayland",
        "user",
        "enp6s0",
        "1999307276288",
        "65781248",
        false,
        1,
    ),
    template(
        "ubuntu-precision7875-7975wx",
        "linux-workstation",
        "precision-tr",
        "Precision 7875 Tower",
        "Dell Precision 7875 Tower",
        "Linux 6.8.0-60-generic x86_64",
        "Ubuntu 24.04 LTS",
        "GNOME",
        "x11",
        "workstation",
        "enp4s0",
        "1999307276288",
        "131595264",
        false,
        2,
    ),
    template(
        "ubuntu-precision5860-xeonw7",
        "linux-workstation",
        "precision-xeon",
        "Precision 5860 Tower",
        "Dell Precision 5860 Tower",
        "Linux 6.8.0-60-generic x86_64",
        "Ubuntu 24.04 LTS",
        "GNOME",
        "x11",
        "workstation",
        "enp4s0",
        "1999307276288",
        "131595264",
        false,
        1,
    ),
    template(
        "rocky-h12ssl-epyc7443p",
        "linux-workstation",
        "epyc-ws",
        "H12SSL-i",
        "Supermicro H12SSL-i",
        "Linux 5.14.0-427.24.1.el9_4.x86_64 x86_64",
        "Rocky Linux 9.4",
        "GNOME",
        "x11",
        "workstation",
        "enp65s0f0",
        "1999307276288",
        "131595264",
        false,
        1,
    ),
];

#[allow(clippy::too_many_arguments)]
const fn template(
    archetype: &'static str,
    category: &'static str,
    host_prefix: &'static str,
    device_name: &'static str,
    hardware_model: &'static str,
    os_release: &'static str,
    distro: &'static str,
    desktop_environment: &'static str,
    session_type: &'static str,
    login_name: &'static str,
    network_interface: &'static str,
    root_filesystem_bytes: &'static str,
    memory_total_kib: &'static str,
    portable: bool,
    weight: u16,
) -> SyntheticTemplate {
    SyntheticTemplate {
        archetype,
        category,
        host_prefix,
        device_name,
        hardware_model,
        os_release,
        distro,
        desktop_environment,
        session_type,
        login_name,
        network_interface,
        root_filesystem_bytes,
        memory_total_kib,
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
    let hostname = format!(
        "{}-{}",
        template.host_prefix,
        10 + u16::from(random[21]) % 89
    );
    let portrait = DevicePortrait::new(
        template.archetype.to_owned(),
        template.category.to_owned(),
        hostname,
        template.device_name.to_owned(),
        template.hardware_model.to_owned(),
        template.os_release.to_owned(),
        template.distro.to_owned(),
        template.desktop_environment.to_owned(),
        template.session_type.to_owned(),
        template.login_name.to_owned(),
        template.network_interface.to_owned(),
        template.root_filesystem_bytes.to_owned(),
        template.memory_total_kib.to_owned(),
        "Asia/Shanghai".to_owned(),
    )
    .map_err(|_error| invalid())?;
    DeviceProfile::from_portrait(guid, mac_address, portrait, power).map_err(|_error| invalid())
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

#[cfg(test)]
mod tests {
    use super::{TEMPLATES, select_template};

    #[test]
    fn frozen_template_weights_and_order_are_complete() {
        assert_eq!(TEMPLATES.len(), 17);
        assert_eq!(TEMPLATES.iter().map(|value| value.weight).sum::<u16>(), 96);
        assert_eq!(select_template(0).archetype, "uos-m720q");
        assert_eq!(select_template(95).archetype, "rocky-h12ssl-epyc7443p");
    }
}
