use qq_domain::DevicePower;

use super::{load_or_generate, schema::format_guid};

#[test]
fn generates_then_reuses_user_managed_profile() -> Result<(), Box<dyn std::error::Error>> {
    let temporary = tempfile::tempdir()?;
    let path = temporary.path().join("device.json");
    let first = load_or_generate(&path)?;
    let second = load_or_generate(&path)?;
    assert_eq!(first, second);
    assert_eq!(first.device_name(), first.model());
    assert!(path.is_file());
    let document: serde_json::Value = serde_json::from_slice(&std::fs::read(&path)?)?;
    let fields = document
        .as_object()
        .ok_or("generated device profile is not an object")?;
    assert_eq!(fields.len(), 8);
    for required in [
        "schema_version",
        "guid",
        "mac_address",
        "device_name",
        "model",
        "system_kernel",
        "kernel_version",
        "power",
    ] {
        assert!(fields.contains_key(required));
    }
    assert!(!fields.contains_key("hostname"));
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
            "schema_version":3,
            "guid":"01010101-0101-4101-8101-010101010101",
            "mac_address":"02:00:00:00:00:01",
            "device_name":"Dell XPS 13 9310",
            "model":"Dell XPS 13 9310",
            "system_kernel":"Linux 5.15.0-139-generic",
            "kernel_version":"5.15.0-139-generic",
            "power":{"kind":"portable","percent":85,"charging":true}
        }"#,
    )?;
    let device = load_or_generate(&path)?;
    assert_eq!(device.device_name(), "Dell XPS 13 9310");
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
            "schema_version":3,
            "guid":"00112233-4455-6677-8899-aabbccddeeff",
            "mac_address":"02:11:22:33:44:55",
            "device_name":"Lenovo ThinkCentre M720q",
            "model":"Lenovo ThinkCentre M720q",
            "system_kernel":"Linux 5.15.0-139-generic",
            "kernel_version":"5.15.0-139-generic",
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
            "schema_version":3,
            "guid":"01010101-0101-4101-8101-010101010101",
            "mac_address":"02:00:00:00:00:01",
            "device_name":"Synthetic desktop",
            "model":"Synthetic desktop",
            "system_kernel":"Linux",
            "kernel_version":"6.8.0-generic",
            "power":{"kind":"desktop"},
            "private_material":"forbidden"
        }"#,
    )?;
    assert!(load_or_generate(&path).is_err());
    Ok(())
}
