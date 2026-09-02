use qq_domain::DevicePower;

use super::{load_or_generate, schema::format_guid};

#[test]
fn generates_then_reuses_user_managed_profile() -> Result<(), Box<dyn std::error::Error>> {
    let temporary = tempfile::tempdir()?;
    let path = temporary.path().join("device.json");
    let first = load_or_generate(&path)?;
    let second = load_or_generate(&path)?;
    assert_eq!(first, second);
    assert!(path.is_file());
    Ok(())
}

#[test]
fn accepts_external_profile_fields() -> Result<(), Box<dyn std::error::Error>> {
    let temporary = tempfile::tempdir()?;
    let path = temporary.path().join("device.json");
    std::fs::write(
        &path,
        br#"{
            "schema_version":1,
            "guid":"01010101-0101-4101-8101-010101010101",
            "mac_address":"02:00:00:00:00:01",
            "name":"Lirvena device",
            "model":"Synthetic portable",
            "system_kernel":"Linux",
            "kernel_version":"6.8.0-generic",
            "power":{"kind":"portable","percent":85,"charging":true}
        }"#,
    )?;
    let device = load_or_generate(&path)?;
    assert_eq!(device.name(), "Lirvena device");
    assert_eq!(device.mac_address(), &[2, 0, 0, 0, 0, 1]);
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
fn rejects_unknown_private_field() -> Result<(), Box<dyn std::error::Error>> {
    let temporary = tempfile::tempdir()?;
    let path = temporary.path().join("device.json");
    std::fs::write(
        &path,
        br#"{
            "schema_version":1,
            "guid":"01010101-0101-4101-8101-010101010101",
            "mac_address":"02:00:00:00:00:01",
            "name":"Lirvena device",
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
