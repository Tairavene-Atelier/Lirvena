//! Offline QR-fetch packet construction tests.

use ceylith_protocol::{OpaqueSlots, ProfileId};
use qq_domain::{DevicePower, DeviceProfile};
use qq_envelope::{
    QqTeaKey, ServiceFrameParts, SsoRequestParts, decrypt_qq_tea, encode_service_frame,
    encode_sso_request,
};
use qq_login::{QqKeyAgreement, QrDevice, QrFetchContext, build_qr_fetch};
use qq_profile::{LinuxNtProfile, LinuxNtProfileSpec};
use qq_wire::{LengthPrefix, WireReader};

struct FakeAgreement {
    public_key: [u8; 25],
    tea_key: QqTeaKey,
}

impl QqKeyAgreement for FakeAgreement {
    fn public_key(&self) -> &[u8] {
        &self.public_key
    }

    fn tea_key(&self) -> &QqTeaKey {
        &self.tea_key
    }
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

#[test]
fn fetch_body_has_bounded_wtlogin_and_encrypted_transaction()
-> Result<(), Box<dyn std::error::Error>> {
    let profile = profile()?;
    let device = QrDevice::new(DeviceProfile::new(
        [0x11; 16],
        [2, 0, 0, 0, 0, 1],
        "Lirvena-Test".to_owned(),
        "Synthetic desktop".to_owned(),
        "Linux".to_owned(),
        "6.8.0-generic".to_owned(),
        DevicePower::Desktop,
    )?);
    let agreement = FakeAgreement {
        public_key: [2; 25],
        tea_key: QqTeaKey::new([3; 16]),
    };
    let random_key = QqTeaKey::new([4; 16]);
    let request = build_qr_fetch(QrFetchContext {
        profile: &profile,
        device: &device,
        sso_sequence: 10,
        unix_seconds: 1_700_000_000,
        random_key: &random_key,
        key_agreement: &agreement,
    })?;
    assert_eq!(request.command(), "wtlogin.trans_emp");
    assert_eq!(request.sequence(), 10);
    assert_eq!(request.payload().len(), 356);

    let mut packet = WireReader::new(request.payload());
    assert_eq!(packet.read_u8()?, 2);
    assert_eq!(usize::from(packet.read_u16()?), request.payload().len());
    assert_eq!(packet.read_u16()?, 8_001);
    assert_eq!(packet.read_u16()?, 2_066);
    assert_eq!(packet.read_u16()?, 0);
    assert_eq!(packet.read_u32()?, 0);
    assert_eq!(packet.read_u8()?, 3);
    assert_eq!(packet.read_u8()?, 135);
    assert_eq!(packet.read_u32()?, 0);
    assert_eq!(packet.read_u8()?, 2);
    assert_eq!(packet.read_u16()?, 0);
    assert_eq!(packet.read_u16()?, 456);
    assert_eq!(packet.read_u32()?, 0);
    assert_eq!(packet.read_u8()?, 1);
    assert_eq!(packet.read_u8()?, 1);
    assert_eq!(packet.read_bytes(16)?, [4; 16]);
    assert_eq!(packet.read_u16()?, 0x102);
    assert_eq!(
        packet.read_prefixed_bytes(LengthPrefix::U16Payload, 128)?,
        [2; 25]
    );
    let encrypted = packet.read_bytes(packet.remaining() - 1)?;
    assert_eq!(packet.read_u8()?, 3);
    packet.finish()?;

    let data = decrypt_qq_tea(encrypted, agreement.tea_key())?;
    let mut data_reader = WireReader::new(&data);
    assert_eq!(data_reader.read_u8()?, 0);
    let request_len = usize::from(data_reader.read_u16()?);
    assert_eq!(data_reader.read_u32()?, profile.app_id());
    assert_eq!(data_reader.read_u32()?, 0x72);
    assert!(
        data_reader
            .read_prefixed_bytes(LengthPrefix::U16Payload, 0)?
            .is_empty()
    );
    assert!(
        data_reader
            .read_prefixed_bytes(LengthPrefix::U8Payload, 0)?
            .is_empty()
    );
    assert_eq!(request_len, data_reader.remaining());
    assert_eq!(data_reader.read_u32()?, 1_700_000_000);
    assert_eq!(data_reader.read_u8()?, 2);
    Ok(())
}

#[test]
fn unsigned_body_accepts_injected_reserve_before_outer_encryption()
-> Result<(), Box<dyn std::error::Error>> {
    let profile = profile()?;
    let device = QrDevice::new(DeviceProfile::new(
        [0x22; 16],
        [2, 0, 0, 0, 0, 2],
        "Lirvena-Test".to_owned(),
        "Synthetic desktop".to_owned(),
        "Linux".to_owned(),
        "6.8.0-generic".to_owned(),
        DevicePower::Desktop,
    )?);
    let agreement = FakeAgreement {
        public_key: [3; 25],
        tea_key: QqTeaKey::new([4; 16]),
    };
    let random_key = QqTeaKey::new([5; 16]);
    let request = build_qr_fetch(QrFetchContext {
        profile: &profile,
        device: &device,
        sso_sequence: 12,
        unix_seconds: 1_700_000_001,
        random_key: &random_key,
        key_agreement: &agreement,
    })?;
    let sso = encode_sso_request(SsoRequestParts {
        sequence: request.sequence(),
        sub_app_id: profile.sub_app_id(),
        locale_id: 2_052,
        tgt: &[],
        command: request.command(),
        device_guid_hex: b"22222222222222222222222222222222",
        client_version: profile.client_version(),
        reserve: &[7, 8, 9],
        payload: request.payload(),
    })?;
    let zero_key = QqTeaKey::new([0; 16]);
    let service = encode_service_frame(ServiceFrameParts {
        uin: 0,
        d2: &[],
        d2_key: &zero_key,
        sso: &sso,
    })?;
    assert!(service.len() > sso.len());
    assert!(!format!("{request:?}").contains("222222"));
    Ok(())
}
