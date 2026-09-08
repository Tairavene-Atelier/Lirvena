//! Protocol-12 SSO and service-envelope layout tests.

mod common;

use qq_envelope::{
    QqTeaKey, ServiceFrameParts, SsoRequestParts, decode_service_response, decode_sso_response,
    decrypt_qq_tea, encode_service_frame, encode_sso_request, encrypt_qq_tea,
};
use qq_wire::{LengthPrefix, WireReader, WireWriter};

use common::decode_hex;

#[test]
fn sso_request_matches_frozen_csharp_packer_byte_for_byte() -> Result<(), Box<dyn std::error::Error>>
{
    // Produced offline by the audited 52194 C# SsoPacker with all ordinary
    // inputs fixed. The trace is random upstream, so its serialized bytes are
    // captured as part of the reserve instead of being regenerated here.
    let reserve = decode_hex(
        "58841070017a3730302d64626131353264666539633732396534366232626666356363646163353035362d646265656662623963383531356562622d303182010e755f66726f7a656e5f3532313934900100980101a00101c201280a1000112233445566778899aabbccddeeff120c66726f7a656e2d746f6b656e1a06313233343536d00164",
    )?;
    let expected = decode_hex(
        "00000105102030402007c2770000080402000000000000000000000000000010a0a1a2a3a4a5a6a7a8a9aaab0000001577746c6f67696e2e7472616e735f656d700000000400000024333332323131303035353434373736363838393961616262636364646565666600000004000e332e322e33322d35323139340000008a58841070017a3730302d64626131353264666539633732396534366232626666356363646163353035362d646265656662623963383531356562622d303182010e755f66726f7a656e5f3532313934900100980101a00101c201280a1000112233445566778899aabbccddeeff120c66726f7a656e2d746f6b656e1a06313233343536d001640000000d010203040506070809",
    )?;
    let actual = encode_sso_request(SsoRequestParts {
        sequence: 0x1020_3040,
        sub_app_id: 537_379_447,
        locale_id: 2_052,
        tgt: &[
            0xa0, 0xa1, 0xa2, 0xa3, 0xa4, 0xa5, 0xa6, 0xa7, 0xa8, 0xa9, 0xaa, 0xab,
        ],
        command: "wtlogin.trans_emp",
        device_guid_hex: b"33221100554477668899aabbccddeeff",
        client_version: "3.2.32-52194",
        reserve: &reserve,
        payload: &[1, 2, 3, 4, 5, 6, 7, 8, 9],
    })?;
    assert_eq!(actual, expected);
    Ok(())
}

#[test]
fn frozen_csharp_service_frames_decrypt_to_the_exact_sso() -> Result<(), Box<dyn std::error::Error>>
{
    let expected_sso = decode_hex(
        "00000105102030402007c2770000080402000000000000000000000000000010a0a1a2a3a4a5a6a7a8a9aaab0000001577746c6f67696e2e7472616e735f656d700000000400000024333332323131303035353434373736363838393961616262636364646565666600000004000e332e322e33322d35323139340000008a58841070017a3730302d62653836653861386361386531343864666130643932636638623033316232642d366362393938383835623633343562642d303182010e755f66726f7a656e5f3532313934900100980101a00101c201280a1000112233445566778899aabbccddeeff120c66726f7a656e2d746f6b656e1a06313233343536d001640000000d010203040506070809",
    )?;
    let anonymous = decode_hex(
        "000001330000000c0200000004000000000530b11beacb76bde5ba051f5f5e89fc3f273e7bd6546cdf4ad0ca125accc4b5ce68437d7852cd252efb823fa351a2058d7d74d12675729ef726376bcd988e26692989da50ff354deebea9d0cb0e59bca3a7c4535f5a59dfc6688fa119a58b3c6f43ffcda6b4268ca3962c37aaa99c9b8f41d005e24624c6a08ef541e9397f4b43dd0aeffa544dd8fc9b760d316cd3674b0eff831e2daa63f02e7f74f0fb4b6733a12fc2a263cfa8136d6ff722563eecfb621b8d68db666b0feff1d961b583f2dcbd9d0362d2adb7e326b105d2e2b17357aa26b68bdec9cfe7330ba179201d27dfa108912f5eaadf86e65be74b8fbd3a5bbdac8ebb61daa4591c72fef8ce5545bd2973e94baaa16c1bad0911b670fa16a17103f323b6190b6349c2f4b3bc229c9d98",
    )?;
    let authenticated = decode_hex(
        "000001440000000c010000000cd0d1d2d3d4d5d6d7000000000e31323334353637383930c4ba42caa2459e43a89cd8da604ee650ba5f9699b7181be5fb971214ad2bb6914be28db3cbc7e68cb966bda9eaab218a395093d1819210da249621524a6b4ad7ec08589e5ab4439f00b828ee80e7ccce4fae51ebd683788676a24d0bbbb69ea485a508e01172529b4ff6c9e3ecfb572ee6b93f4097ef4f4a4a6b6b0e9fceaa3b51df56d9f1375af6eb55853ecb8def5efa9af41c60b97b13413a07bc287401b6594f08477cfd17496092e09a7dbc7fead723f33fb3b8c61157e41b27eff924b346ca111e07492e998c82d7543be710fe628be65eed64f8228db7edac47d544cc6e1b21240c0f6b15044b596151f745157f7350a00424ec94d716562c134cd09bd089bce8748aaf7717799c319b92d8efb20945b7eb7e167dd4720deaae54d73a",
    )?;
    assert_frozen_service_request(
        &anonymous,
        &[],
        b"0",
        &QqTeaKey::new([0; 16]),
        &expected_sso,
    )?;
    assert_frozen_service_request(
        &authenticated,
        &[0xd0, 0xd1, 0xd2, 0xd3, 0xd4, 0xd5, 0xd6, 0xd7],
        b"1234567890",
        &QqTeaKey::new([
            0xe0, 0xe1, 0xe2, 0xe3, 0xe4, 0xe5, 0xe6, 0xe7, 0xe8, 0xe9, 0xea, 0xeb, 0xec, 0xed,
            0xee, 0xef,
        ]),
        &expected_sso,
    )?;
    Ok(())
}

fn assert_frozen_service_request(
    frame: &[u8],
    expected_d2: &[u8],
    expected_uin: &[u8],
    key: &QqTeaKey,
    expected_sso: &[u8],
) -> Result<(), Box<dyn std::error::Error>> {
    let mut outer = WireReader::new(frame);
    let body = outer.read_prefixed_bytes(LengthPrefix::U32Inclusive, 4_096)?;
    outer.finish()?;
    let mut body = WireReader::new(body);
    assert_eq!(body.read_u32()?, 12);
    assert_eq!(body.read_u8()?, if expected_d2.is_empty() { 2 } else { 1 });
    assert_eq!(
        body.read_prefixed_bytes(LengthPrefix::U32Inclusive, 64)?,
        expected_d2
    );
    assert_eq!(body.read_u8()?, 0);
    assert_eq!(
        body.read_prefixed_bytes(LengthPrefix::U32Inclusive, 32)?,
        expected_uin
    );
    let encrypted = body.read_bytes(body.remaining())?;
    assert_eq!(decrypt_qq_tea(encrypted, key)?, expected_sso);
    body.finish()?;
    Ok(())
}

#[test]
fn sso_request_has_two_strict_inclusive_sections() -> Result<(), Box<dyn std::error::Error>> {
    let parts = SsoRequestParts {
        sequence: 7,
        sub_app_id: 8,
        locale_id: 2_052,
        tgt: &[],
        command: "wtlogin.trans_emp",
        device_guid_hex: b"00112233445566778899aabbccddeeff",
        client_version: "1.2.3-456",
        reserve: &[9, 10],
        payload: &[11, 12, 13],
    };
    let encoded = encode_sso_request(parts)?;
    let mut reader = WireReader::new(&encoded);
    let header = reader.read_prefixed_bytes(LengthPrefix::U32Inclusive, 2_048)?;
    assert_eq!(
        reader.read_prefixed_bytes(LengthPrefix::U32Inclusive, 32)?,
        [11, 12, 13]
    );
    reader.finish()?;

    let mut header_reader = WireReader::new(header);
    assert_eq!(header_reader.read_u32()?, 7);
    assert_eq!(header_reader.read_u32()?, 8);
    assert_eq!(header_reader.read_u32()?, 2_052);
    Ok(())
}

#[test]
fn anonymous_service_frame_uses_zero_key_and_uin() -> Result<(), Box<dyn std::error::Error>> {
    let supplied_key = QqTeaKey::new([9; 16]);
    let encoded = encode_service_frame(ServiceFrameParts {
        uin: 0,
        d2: &[],
        d2_key: &supplied_key,
        sso: b"signed-sso",
    })?;
    let mut outer = WireReader::new(&encoded);
    let body = outer.read_prefixed_bytes(LengthPrefix::U32Inclusive, 4_096)?;
    outer.finish()?;
    let mut body_reader = WireReader::new(body);
    assert_eq!(body_reader.read_u32()?, 12);
    assert_eq!(body_reader.read_u8()?, 2);
    assert!(
        body_reader
            .read_prefixed_bytes(LengthPrefix::U32Inclusive, 16)?
            .is_empty()
    );
    assert_eq!(body_reader.read_u8()?, 0);
    assert_eq!(
        body_reader.read_prefixed_bytes(LengthPrefix::U32Inclusive, 8)?,
        b"0"
    );
    let encrypted = body_reader.read_bytes(body_reader.remaining())?;
    assert_eq!(
        decrypt_qq_tea(encrypted, &QqTeaKey::new([0; 16]))?,
        b"signed-sso"
    );
    body_reader.finish()?;
    Ok(())
}

#[test]
fn inbound_service_and_sso_responses_decode_strictly() -> Result<(), Box<dyn std::error::Error>> {
    let mut sso_header = WireWriter::new(4_096);
    sso_header.put_u32(17)?;
    sso_header.put_u32(0)?;
    sso_header.put_prefixed_bytes(LengthPrefix::U32Inclusive, &[])?;
    sso_header.put_prefixed_bytes(LengthPrefix::U32Inclusive, b"wtlogin.trans_emp")?;
    sso_header.put_prefixed_bytes(LengthPrefix::U32Inclusive, &[])?;
    sso_header.put_u32(0)?;
    sso_header.put_prefixed_bytes(LengthPrefix::U32Inclusive, &[1, 2])?;
    sso_header.put_bytes(&[6, 7, 8, 9])?;
    let mut sso = WireWriter::new(4_096);
    sso.put_prefixed_bytes(LengthPrefix::U32Inclusive, &sso_header.finish())?;
    sso.put_prefixed_bytes(LengthPrefix::U32Inclusive, &[3, 4, 5])?;
    let sso = sso.finish();

    let zero_key = QqTeaKey::new([0; 16]);
    let encrypted = encrypt_qq_tea(&sso, &zero_key)?;
    let mut service_body = WireWriter::new(8_192);
    service_body.put_u32(12)?;
    service_body.put_u8(2)?;
    service_body.put_u8(0)?;
    service_body.put_prefixed_bytes(LengthPrefix::U32Inclusive, b"0")?;
    service_body.put_bytes(&encrypted)?;
    let mut service = WireWriter::new(8_192);
    service.put_prefixed_bytes(LengthPrefix::U32Inclusive, &service_body.finish())?;

    let service = decode_service_response(&service.finish(), None)?;
    assert_eq!(service.uin(), "0");
    let sso = decode_sso_response(service.payload())?;
    assert_eq!(sso.sequence(), 17);
    assert_eq!(sso.command(), "wtlogin.trans_emp");
    assert_eq!(sso.reserve(), [1, 2]);
    assert_eq!(sso.extension(), [6, 7, 8, 9]);
    assert_eq!(sso.payload(), [3, 4, 5]);
    Ok(())
}
