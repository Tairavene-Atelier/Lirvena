use qq_domain::DevicePower;

use super::{load_or_generate, schema::format_guid};

#[test]
fn generates_then_reuses_user_managed_profile() -> Result<(), Box<dyn std::error::Error>> {
    let temporary = tempfile::tempdir()?;
    let path = temporary.path().join("device.json");
    let first = load_or_generate(&path)?;
    let second = load_or_generate(&path)?;
    assert_eq!(first, second);
    assert_ne!(first.device_name(), first.model());
    assert_eq!(first.device_name(), first.portrait().hostname());
    assert!(path.is_file());
    let document: serde_json::Value = serde_json::from_slice(&std::fs::read(&path)?)?;
    let fields = document
        .as_object()
        .ok_or("generated device profile is not an object")?;
    assert_eq!(fields.len(), 18);
    for required in [
        "schema_version",
        "guid",
        "mac_address",
        "archetype",
        "profile_category",
        "hostname",
        "device_name",
        "hardware_model",
        "os_release",
        "distro",
        "desktop_environment",
        "session_type",
        "login_name",
        "network_interface",
        "root_filesystem_bytes",
        "memory_total_kib",
        "timezone",
        "power",
    ] {
        assert!(fields.contains_key(required));
    }
    assert!(!fields.contains_key("qua"));
    assert!(!fields.contains_key("version"));
    Ok(())
}

#[test]
fn accepts_external_profile_fields() -> Result<(), Box<dyn std::error::Error>> {
    let temporary = tempfile::tempdir()?;
    let path = temporary.path().join("device.json");
    std::fs::write(
        &path,
        br#"{
            "schema_version":4,
            "guid":"01010101-0101-4101-8101-010101010101",
            "mac_address":"02:00:00:00:00:01",
            "archetype":"ubuntu-xps9310-developer",
            "profile_category":"linux-preinstalled",
            "hostname":"xps-dev-42",
            "device_name":"XPS 13 9310",
            "hardware_model":"Dell XPS 13 9310",
            "os_release":"Linux 5.15.0-139-generic x86_64",
            "distro":"Ubuntu 20.04.6 LTS",
            "desktop_environment":"GNOME",
            "session_type":"x11",
            "login_name":"developer",
            "network_interface":"wlp0s20f3",
            "root_filesystem_bytes":"255852544000",
            "memory_total_kib":"7864320",
            "timezone":"Asia/Shanghai",
            "power":{"kind":"portable","percent":85,"charging":true}
        }"#,
    )?;
    let device = load_or_generate(&path)?;
    assert_eq!(device.device_name(), "xps-dev-42");
    assert_eq!(device.portrait().device_name(), "XPS 13 9310");
    assert_eq!(device.mac_address(), &[2, 0, 0, 0, 0, 1]);
    assert_eq!(
        device.guid(),
        &[1, 1, 1, 1, 1, 1, 1, 0x41, 0x81, 1, 1, 1, 1, 1, 1, 1]
    );
    assert_eq!(
        device.power(),
        DevicePower::Portable {
            percent: 85,
            charging: true
        }
    );
    assert_eq!(
        format_guid(device.guid()),
        "01010101-0101-4101-8101-010101010101"
    );
    Ok(())
}

#[test]
fn preserves_frozen_dotnet_guid_wire_order() -> Result<(), Box<dyn std::error::Error>> {
    let temporary = tempfile::tempdir()?;
    let path = temporary.path().join("device.json");
    std::fs::write(
        &path,
        br#"{
            "schema_version":4,
            "guid":"00112233-4455-6677-8899-aabbccddeeff",
            "mac_address":"02:11:22:33:44:55",
            "archetype":"uos-m720q",
            "profile_category":"domestic-enterprise",
            "hostname":"uos-office-42",
            "device_name":"ThinkCentre M720q",
            "hardware_model":"Lenovo ThinkCentre M720q",
            "os_release":"Linux 5.10.0-amd64-desktop x86_64",
            "distro":"UOS Desktop 20",
            "desktop_environment":"DDE",
            "session_type":"x11",
            "login_name":"user",
            "network_interface":"enp0s31f6",
            "root_filesystem_bytes":"499963174912",
            "memory_total_kib":"16325684",
            "timezone":"Asia/Shanghai",
            "power":{"kind":"desktop"}
        }"#,
    )?;
    let device = load_or_generate(&path)?;
    assert_eq!(
        device.guid(),
        &[
            0x33, 0x22, 0x11, 0x00, 0x55, 0x44, 0x77, 0x66, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd,
            0xee, 0xff,
        ]
    );
    assert_eq!(
        format_guid(device.guid()),
        "00112233-4455-6677-8899-aabbccddeeff"
    );
    Ok(())
}

#[test]
fn rejects_unknown_private_field() -> Result<(), Box<dyn std::error::Error>> {
    let temporary = tempfile::tempdir()?;
    let path = temporary.path().join("device.json");
    std::fs::write(
        &path,
        br#"{
            "schema_version":4,
            "guid":"01010101-0101-4101-8101-010101010101",
            "mac_address":"02:00:00:00:00:01",
            "archetype":"custom",
            "profile_category":"custom",
            "hostname":"synthetic-desktop-42",
            "device_name":"Synthetic desktop",
            "hardware_model":"Synthetic desktop",
            "os_release":"Linux 6.8.0-generic x86_64",
            "distro":"Linux",
            "desktop_environment":"GNOME",
            "session_type":"x11",
            "login_name":"user",
            "network_interface":"eth0",
            "root_filesystem_bytes":"499963174912",
            "memory_total_kib":"16325684",
            "timezone":"Asia/Shanghai",
            "power":{"kind":"desktop"},
            "private_material":"forbidden"
        }"#,
    )?;
    assert!(load_or_generate(&path).is_err());
    Ok(())
}
