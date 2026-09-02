//! Protocol-12 SSO and service-envelope layout tests.

use qq_envelope::{
    QqTeaKey, ServiceFrameParts, SsoRequestParts, decode_service_response, decode_sso_response,
    decrypt_qq_tea, encode_service_frame, encode_sso_request, encrypt_qq_tea,
};
use qq_wire::{LengthPrefix, WireReader, WireWriter};

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
