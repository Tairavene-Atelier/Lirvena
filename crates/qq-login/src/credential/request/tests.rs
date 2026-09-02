use ceylith_protocol::{OpaqueSlots, ProfileId};
use qq_domain::{DevicePower, DeviceProfile};
use qq_envelope::{QqTeaKey, decrypt_qq_tea};
use qq_profile::{LinuxNtProfile, LinuxNtProfileSpec};
use qq_wire::{LengthPrefix, WireReader};

use super::{CredentialExchangeContext, build_credential_exchange};
use crate::{QqKeyAgreement, QrDevice, QrLoginSecrets};

struct FakeAgreement(QqTeaKey);

impl QqKeyAgreement for FakeAgreement {
    fn public_key(&self) -> &[u8] {
        &[2; 25]
    }

    fn tea_key(&self) -> &QqTeaKey {
        &self.0
    }
}

#[test]
fn request_matches_52194_login_envelope_and_tlv_order() -> Result<(), Box<dyn std::error::Error>> {
    let profile = profile()?;
    let device = QrDevice::new(DeviceProfile::new(
        [4; 16],
        [2, 0, 0, 0, 0, 4],
        "Lirvena test".to_owned(),
        "Synthetic desktop".to_owned(),
        "Linux".to_owned(),
        "6.8.0-generic".to_owned(),
        DevicePower::Desktop,
    )?);
    let secrets = QrLoginSecrets::for_test(10_001, vec![5; 16], vec![6; 32], vec![7; 24]);
    let agreement = FakeAgreement(QqTeaKey::new([8; 16]));
    let random_key = QqTeaKey::new([9; 16]);
    let request = build_credential_exchange(CredentialExchangeContext {
        profile: &profile,
        device: &device,
        sso_sequence: 72,
        random_key: &random_key,
        key_agreement: &agreement,
        secrets: &secrets,
    })?;
    assert_eq!(request.sequence(), 72);
    assert_eq!(request.uin(), 10_001);
    assert_eq!(request.command(), "wtlogin.login");

    let mut reader = WireReader::new(request.payload());
    assert_eq!(reader.read_u8()?, 2);
    assert_eq!(usize::from(reader.read_u16()?), request.payload().len());
    assert_eq!(reader.read_u16()?, 8_001);
    assert_eq!(reader.read_u16()?, 2_064);
    assert_eq!(reader.read_u16()?, 0);
    assert_eq!(reader.read_u32()?, 10_001);
    assert_eq!(reader.read_u8()?, 3);
    assert_eq!(reader.read_u8()?, 135);
    assert_eq!(reader.read_u32()?, 0);
    assert_eq!(reader.read_u8()?, 19);
    assert_eq!(reader.read_u16()?, 0);
    assert_eq!(reader.read_u16()?, 456);
    assert_eq!(reader.read_u32()?, 0);
    assert_eq!(reader.read_u8()?, 1);
    assert_eq!(reader.read_u8()?, 1);
    assert_eq!(reader.read_bytes(16)?, [9; 16]);
    assert_eq!(reader.read_u16()?, 0x102);
    assert_eq!(
        reader.read_prefixed_bytes(LengthPrefix::U16Payload, 64)?,
        [2; 25]
    );
    let encrypted = reader.read_bytes(reader.remaining() - 1)?;
    assert_eq!(reader.read_u8()?, 3);
    reader.finish()?;

    let plaintext = decrypt_qq_tea(encrypted, agreement.tea_key())?;
    let mut plaintext = WireReader::new(&plaintext);
    assert_eq!(plaintext.read_u16()?, 0x09);
    assert_eq!(plaintext.read_u16()?, 15);
    let expected = [
        0x106, 0x144, 0x116, 0x142, 0x145, 0x018, 0x141, 0x177, 0x191, 0x100, 0x107, 0x318, 0x16a,
        0x166, 0x521,
    ];
    let mut encrypted_device_bundle = None;
    for tag in expected {
        assert_eq!(plaintext.read_u16()?, tag);
        let body = plaintext.read_prefixed_bytes(LengthPrefix::U16Payload, 16 * 1024)?;
        assert!(!body.is_empty() || tag == 0x318);
        if tag == 0x144 {
            encrypted_device_bundle = Some(body.to_vec());
        }
    }
    plaintext.finish()?;
    let bundle = decrypt_qq_tea(
        &encrypted_device_bundle.ok_or("missing encrypted device bundle")?,
        &QqTeaKey::new([5; 16]),
    )?;
    let mut bundle = WireReader::new(&bundle);
    assert_eq!(bundle.read_u16()?, 4);
    for tag in [0x16e, 0x147, 0x128, 0x124] {
        assert_eq!(bundle.read_u16()?, tag);
        assert!(
            !bundle
                .read_prefixed_bytes(LengthPrefix::U16Payload, 4 * 1024)?
                .is_empty()
        );
    }
    bundle.finish()?;
    Ok(())
}

fn profile() -> Result<LinuxNtProfile, Box<dyn std::error::Error>> {
    Ok(LinuxNtProfile::new(
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
        },
        OpaqueSlots::default(),
    )?)
}
