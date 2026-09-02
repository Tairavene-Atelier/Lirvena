//! Linux NTQQ profile validation tests.

use ceylith_protocol::{OpaqueError, ProfileId};
use qq_profile::{
    LinuxNtProfile, LinuxNtProfileSpec, OpaqueSlot, OpaqueSlotId, OpaqueSlots, ProfileValueError,
    decode_linux_manifest, encode_linux_manifest,
};

fn valid_spec() -> LinuxNtProfileSpec {
    LinuxNtProfileSpec {
        profile_id: ProfileId::from_bytes([9; 16]),
        client_version: "1.2.3-456".to_owned(),
        app_id: 1_001,
        sub_app_id: 1_002,
        qr_app_id: 1_002,
        app_client_version: 456,
        package_name: "example.client".to_owned(),
        operating_system: "Linux".to_owned(),
        pt_version: "1.2.3".to_owned(),
        sso_version: 7,
        misc_bitmap: 0x55,
        login_sdk: "example.login.1".to_owned(),
        main_sig_map: 0x1234,
        sub_sig_map: 0,
        login_misc_bitmap: 0x5678,
        runtime_abi: 2,
    }
}

#[test]
fn signed_manifest_is_canonical_and_rejects_trailing_data() -> Result<(), Box<dyn std::error::Error>>
{
    let slots = OpaqueSlots::new(vec![OpaqueSlot::new(OpaqueSlotId::new(9)?, vec![4, 5])?])?;
    let profile = LinuxNtProfile::new(valid_spec(), slots)?;
    let manifest = encode_linux_manifest(&profile)?;
    let decoded = decode_linux_manifest(&manifest)?;
    assert_eq!(decoded, profile);
    assert_eq!(encode_linux_manifest(&decoded)?, manifest);

    let mut trailing = manifest;
    trailing.push(0);
    assert!(decode_linux_manifest(&trailing).is_err());
    Ok(())
}

#[test]
fn ordinary_fields_and_numeric_slots_validate_once() -> Result<(), Box<dyn std::error::Error>> {
    let slot_id = OpaqueSlotId::new(7)?;
    let slots = OpaqueSlots::new(vec![OpaqueSlot::new(slot_id, vec![3, 4])?])?;
    let profile = LinuxNtProfile::new(valid_spec(), slots)?;
    assert_eq!(profile.client_version(), "1.2.3-456");
    assert_eq!(profile.app_client_version(), 456);
    assert_eq!(
        profile.opaque_slots().get(slot_id).map(OpaqueSlot::value),
        Some(&[3, 4][..])
    );
    Ok(())
}

#[test]
fn opaque_values_are_redacted_and_ids_must_be_unique() -> Result<(), Box<dyn std::error::Error>> {
    let id = OpaqueSlotId::new(1)?;
    let slot = OpaqueSlot::new(id, b"do-not-print".to_vec())?;
    let debug = format!("{slot:?}");
    assert!(!debug.contains("do-not-print"));
    assert_eq!(
        OpaqueSlots::new(vec![slot.clone(), slot]),
        Err(OpaqueError::DuplicateSlot)
    );
    Ok(())
}

#[test]
fn invalid_text_and_zero_numbers_are_rejected() {
    let mut invalid_text = valid_spec();
    invalid_text.login_sdk = "not allowed whitespace".to_owned();
    assert_eq!(
        LinuxNtProfile::new(invalid_text, OpaqueSlots::default()),
        Err(ProfileValueError::InvalidText)
    );

    let mut invalid_number = valid_spec();
    invalid_number.runtime_abi = 0;
    assert_eq!(
        LinuxNtProfile::new(invalid_number, OpaqueSlots::default()),
        Err(ProfileValueError::ZeroNumber)
    );
}
