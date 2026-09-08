use ceylith_protocol::{OpaqueSlots, ProfileId};
use qq_domain::{DevicePower, DeviceProfile};
use qq_profile::{LinuxNtProfile, LinuxNtProfileSpec};

use super::build_fetch_body;
use crate::QrDevice;

#[test]
fn fetch_body_matches_frozen_52194_plaintext() -> Result<(), Box<dyn std::error::Error>> {
    let profile = profile()?;
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
    assert_eq!(
        build_fetch_body(&profile, &device)?,
        decode_hex(
            "00005f5e164f0000000000000000000000000700160043000000005f5e164f2007c27733221100554477668899aabbccddeeff000e636f6d2e74656e63656e742e71710005322e302e30000e636f6d2e74656e63656e742e7171001b001e000000000000000000000003000000040000004800000002000000020000001d000a0100007ffc00000000000033001033221100554477668899aabbccddeeff0035000400000013006600040000001300d1001c0a160a054c696e7578120d756f732d6f66666963652d343222023001"
        )?
    );
    Ok(())
}

fn profile() -> Result<LinuxNtProfile, Box<dyn std::error::Error>> {
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

fn decode_hex(value: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let text = core::str::from_utf8(pair)?;
            Ok(u8::from_str_radix(text, 16)?)
        })
        .collect()
}
