//! Offline QR-fetch response validation tests.

use prost::Message;
use qq_envelope::{QqTeaKey, encrypt_qq_tea};
use qq_login::{QqKeyAgreement, QrResponseContext, decode_qr_fetch_response};
use qq_wire::{LengthPrefix, WireWriter};

struct FakeAgreement(QqTeaKey);

impl QqKeyAgreement for FakeAgreement {
    fn public_key(&self) -> &[u8] {
        &[2; 25]
    }

    fn tea_key(&self) -> &QqTeaKey {
        &self.0
    }
}

#[derive(Clone, PartialEq, Message)]
struct ResponseInfo {
    #[prost(string, tag = "2")]
    url: String,
    #[prost(string, tag = "3")]
    query_signature: String,
}

#[test]
fn fetch_response_yields_redacted_artifact_and_challenge() -> Result<(), Box<dyn std::error::Error>>
{
    let agreement = FakeAgreement(QqTeaKey::new([7; 16]));
    let random_key = QqTeaKey::new([6; 16]);
    let payload = response_packet(&agreement, 0)?;
    let response = decode_qr_fetch_response(
        &payload,
        QrResponseContext {
            app_id: 1_001,
            issued_at_ms: 1_000,
            random_key: &random_key,
            key_agreement: &agreement,
        },
    )?;
    assert_eq!(response.artifact().url(), "https://example.invalid/qr?id=9");
    assert_eq!(response.artifact().expires_at_ms(), 121_000);
    assert_eq!(response.challenge().poll_signature(), [5; 24]);
    assert_eq!(response.challenge().query_signature(), "query-secret");
    let debug = format!("{response:?}");
    assert!(!debug.contains("query-secret"));
    assert!(!debug.contains("example.invalid"));
    Ok(())
}

#[test]
fn nonzero_internal_sequence_is_fail_closed() -> Result<(), Box<dyn std::error::Error>> {
    let agreement = FakeAgreement(QqTeaKey::new([8; 16]));
    let random_key = QqTeaKey::new([9; 16]);
    let payload = response_packet(&agreement, 20)?;
    assert!(
        decode_qr_fetch_response(
            &payload,
            QrResponseContext {
                app_id: 1_001,
                issued_at_ms: 1_000,
                random_key: &random_key,
                key_agreement: &agreement,
            },
        )
        .is_err()
    );
    Ok(())
}

fn response_packet(
    agreement: &FakeAgreement,
    sequence: u16,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    png.extend_from_slice(b"test-image");
    let info = ResponseInfo {
        url: "https://example.invalid/qr?id=9".to_owned(),
        query_signature: "query-secret".to_owned(),
    }
    .encode_to_vec();

    let mut lifetime = WireWriter::new(16);
    lifetime.put_u32(120)?;
    lifetime.put_u16(2)?;
    let mut tlvs = WireWriter::new(4_096);
    tlvs.put_u16(3)?;
    write_tlv(&mut tlvs, 0x017, &png)?;
    write_tlv(&mut tlvs, 0x01c, &lifetime.finish())?;
    write_tlv(&mut tlvs, 0x0d1, &info)?;

    let mut response = WireWriter::new(8_192);
    response.put_u16(0)?;
    response.put_u32(1_001)?;
    response.put_u8(0)?;
    response.put_prefixed_bytes(LengthPrefix::U16Payload, &[5; 24])?;
    response.put_bytes(&tlvs.finish())?;
    let transaction = code2d_transaction(0x31, &response.finish())?;
    let mut code2d = WireWriter::new(8_192);
    code2d.put_u8(0)?;
    code2d.put_u8(0)?;
    code2d.put_u16(u16::try_from(transaction.len())?)?;
    code2d.put_u8(0)?;
    code2d.put_bytes(&transaction)?;
    let encrypted = encrypt_qq_tea(&code2d.finish(), agreement.tea_key())?;

    let mut body = WireWriter::new(16 * 1024);
    body.put_u16(8_001)?;
    body.put_u16(2_066)?;
    body.put_u16(sequence)?;
    body.put_u32(0)?;
    body.put_u8(0)?;
    body.put_u8(0)?;
    body.put_u8(0)?;
    body.put_bytes(&encrypted)?;
    body.put_u8(3)?;
    let body = body.finish();
    let mut packet = WireWriter::new(16 * 1024);
    packet.put_u8(2)?;
    packet.put_u16(u16::try_from(body.len() + 3)?)?;
    packet.put_bytes(&body)?;
    Ok(packet.finish())
}

fn code2d_transaction(
    command: u16,
    response: &[u8],
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut transaction = WireWriter::new(8_192);
    transaction.put_u8(2)?;
    transaction.put_u16(u16::try_from(response.len() + 44)?)?;
    transaction.put_u16(command)?;
    transaction.put_bytes(&[0; 21])?;
    transaction.put_u8(3)?;
    transaction.put_u16(0)?;
    transaction.put_u16(0x32)?;
    transaction.put_u32(0)?;
    transaction.put_u64(0)?;
    transaction.put_bytes(response)?;
    transaction.put_u8(3)?;
    Ok(transaction.finish())
}

fn write_tlv(writer: &mut WireWriter, tag: u16, body: &[u8]) -> Result<(), qq_wire::WireError> {
    writer.put_u16(tag)?;
    writer.put_prefixed_bytes(LengthPrefix::U16Payload, body)
}
