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
        wtlogin_sequence: 0,
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

#[test]
fn request_tlvs_match_frozen_52194_plaintexts() -> Result<(), Box<dyn std::error::Error>> {
    let profile = frozen_profile()?;
    let device = QrDevice::new(DeviceProfile::new(
        [
            0x33, 0x22, 0x11, 0x00, 0x55, 0x44, 0x77, 0x66, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd,
            0xee, 0xff,
        ],
        [0x02, 0x11, 0x22, 0x33, 0x44, 0x55],
        "uos-office-42".to_owned(),
        "Lenovo ThinkCentre M720q".to_owned(),
        "Linux 5.15.0-139-generic".to_owned(),
        "5.15.0-139-generic".to_owned(),
        DevicePower::Desktop,
    )?);
    let secrets = QrLoginSecrets::for_test(
        1_234_567_890,
        (0x10..0x20).collect(),
        (0x20..0x40).collect(),
        (0x40..0x50).collect(),
    );
    let agreement = FakeAgreement(QqTeaKey::new([8; 16]));
    let request = build_credential_exchange(CredentialExchangeContext {
        profile: &profile,
        device: &device,
        sso_sequence: 72,
        wtlogin_sequence: 2,
        random_key: &QqTeaKey::new([9; 16]),
        key_agreement: &agreement,
        secrets: &secrets,
    })?;

    let mut outer = WireReader::new(request.payload());
    outer.read_bytes(3 + 2 + 2 + 2 + 4 + 1 + 1 + 4 + 1 + 2 + 2 + 4 + 1 + 1 + 16 + 2)?;
    outer.read_prefixed_bytes(LengthPrefix::U16Payload, 64)?;
    let encrypted = outer.read_bytes(outer.remaining() - 1)?;
    assert_eq!(outer.read_u8()?, 3);
    let plaintext = decrypt_qq_tea(encrypted, agreement.tea_key())?;
    let mut reader = WireReader::new(&plaintext);
    assert_eq!(reader.read_u16()?, 0x09);
    assert_eq!(reader.read_u16()?, 15);

    let expected = [
        (
            0x106,
            "202122232425262728292a2b2c2d2e2f303132333435363738393a3b3c3d3e3f",
        ),
        (
            0x144,
            "0004016e000d756f732d6f66666963652d34320147001b5f5e164f0005322e302e30000e636f6d2e74656e63656e742e71710128002400000000000000000000054c696e7578001033221100554477668899aabbccddeeff00000124000c000000000000000000000000",
        ),
        (0x116, "0000b7fffc0000000000"),
        (0x142, "0000000e636f6d2e74656e63656e742e7171"),
        (0x145, "33221100554477668899aabbccddeeff"),
        (0x018, "0000000000050000000000001f41499602d200000000"),
        (0x141, "00000007556e6b6e6f776e00000000"),
        (0x177, "010000000000106e742e77746c6f67696e2e302e302e31"),
        (0x191, "00"),
        (0x100, "0000000000055f5e164f2007c2770000cbe20a1e10e0"),
        (0x107, "00010d000001"),
        (0x318, ""),
        (0x16a, "404142434445464748494a4b4c4d4e4f"),
        (0x166, "05"),
        (0x521, "0000001300076261736963696d"),
    ];
    for (tag, expected_hex) in expected {
        assert_eq!(reader.read_u16()?, tag);
        let body = reader.read_prefixed_bytes(LengthPrefix::U16Payload, 16 * 1024)?;
        let actual = if tag == 0x144 {
            decrypt_qq_tea(
                body,
                &QqTeaKey::new([
                    0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b, 0x1c,
                    0x1d, 0x1e, 0x1f,
                ]),
            )?
        } else {
            body.to_vec()
        };
        assert_eq!(actual, decode_hex(expected_hex)?);
    }
    reader.finish()?;
    Ok(())
}

fn decode_hex(value: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    if !value.len().is_multiple_of(2) {
        return Err("odd hexadecimal fixture".into());
    }
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let text = core::str::from_utf8(pair)?;
            Ok(u8::from_str_radix(text, 16)?)
        })
        .collect()
}

fn frozen_profile() -> Result<LinuxNtProfile, Box<dyn std::error::Error>> {
    Ok(LinuxNtProfile::new(
        LinuxNtProfileSpec {
            profile_id: ProfileId::from_bytes([9; 16]),
            client_version: "3.2.32-52194".to_owned(),
            app_id: 1_600_001_615,
            sub_app_id: 537_379_447,
            qr_app_id: 537_379_447,
            app_client_version: 52_194,
            package_name: "com.tencent.qq".to_owned(),
            operating_system: "Linux".to_owned(),
            pt_version: "2.0.0".to_owned(),
            sso_version: 19,
            misc_bitmap: 32_764,
            login_sdk: "nt.wtlogin.0.0.1".to_owned(),
            main_sig_map: 169_742_560,
            sub_sig_map: 0,
            login_misc_bitmap: 12_058_620,
            runtime_abi: 2,
        },
        OpaqueSlots::default(),
    )?)
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
