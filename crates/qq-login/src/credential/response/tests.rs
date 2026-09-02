use prost::Message;
use qq_envelope::{QqTeaKey, encrypt_qq_tea};
use qq_wire::{LengthPrefix, WireWriter};

use super::{
    CredentialExchangeOutcome, CredentialResponseContext, UidEnvelope, UidLayerOne, UidLayerTwo,
    decode_credential_exchange_response,
};
use crate::{KeyAgreementError, QqKeyAgreement};

const LIMIT: usize = 64 * 1024;
const UIN: u32 = 10_001;

struct TestAgreement {
    key: QqTeaKey,
}

impl QqKeyAgreement for TestAgreement {
    fn public_key(&self) -> &[u8] {
        &[]
    }

    fn tea_key(&self) -> &QqTeaKey {
        &self.key
    }

    fn derive_response_key(&self, _peer_public: &[u8]) -> Result<QqTeaKey, KeyAgreementError> {
        Err(KeyAgreementError::InvalidPeer)
    }
}

#[test]
fn decodes_complete_success_and_redacts_session_material() -> Result<(), Box<dyn std::error::Error>>
{
    let agreement = TestAgreement {
        key: QqTeaKey::new([0x41; 16]),
    };
    let tgtgt_key = QqTeaKey::new([0x52; 16]);
    let nested = collection(&[
        (0x11a, profile_body("Lirvena")?),
        (0x305, vec![0x31; 16]),
        (0x543, uid_body("u_test")?),
        (0x10a, vec![0x32; 48]),
        (0x143, vec![0x33; 64]),
        (0x106, vec![0x34; 32]),
    ])?;
    let outer = collection(&[(0x119, encrypt_qq_tea(&nested, &tgtgt_key)?)])?;
    let packet = response_packet(0, &outer, &agreement.key)?;

    let outcome = decode_credential_exchange_response(
        &packet,
        CredentialResponseContext {
            uin: UIN,
            key_agreement: &agreement,
            tgtgt_key: tgtgt_key.as_bytes(),
        },
    )?;
    let CredentialExchangeOutcome::Success(login) = outcome else {
        return Err(std::io::Error::other("expected credential success").into());
    };
    assert_eq!(login.uid(), "u_test");
    assert_eq!(login.nickname(), "Lirvena");
    assert_eq!(login.age(), 18);
    assert_eq!(login.gender(), 2);
    assert_eq!(login.secrets().d2_key(), &[0x31; 16]);
    assert!(!format!("{login:?}").contains("31313131"));
    Ok(())
}

#[test]
fn returns_typed_bounded_rejection() -> Result<(), Box<dyn std::error::Error>> {
    let agreement = TestAgreement {
        key: QqTeaKey::new([0x41; 16]),
    };
    let notice = notice_body("verification", "additional step required")?;
    let packet = response_packet(2, &collection(&[(0x146, notice)])?, &agreement.key)?;
    let outcome = decode_credential_exchange_response(
        &packet,
        CredentialResponseContext {
            uin: UIN,
            key_agreement: &agreement,
            tgtgt_key: &[0x52; 16],
        },
    )?;
    let CredentialExchangeOutcome::Rejected(rejection) = outcome else {
        return Err(std::io::Error::other("expected credential rejection").into());
    };
    assert_eq!(rejection.state(), 2);
    assert_eq!(rejection.tag(), Some("verification"));
    assert_eq!(rejection.message(), Some("additional step required"));
    Ok(())
}

#[test]
fn rejects_success_without_all_session_values() -> Result<(), Box<dyn std::error::Error>> {
    let agreement = TestAgreement {
        key: QqTeaKey::new([0x41; 16]),
    };
    let tgtgt_key = QqTeaKey::new([0x52; 16]);
    let nested = collection(&[(0x305, vec![0x31; 16])])?;
    let outer = collection(&[(0x119, encrypt_qq_tea(&nested, &tgtgt_key)?)])?;
    let packet = response_packet(0, &outer, &agreement.key)?;
    assert!(
        decode_credential_exchange_response(
            &packet,
            CredentialResponseContext {
                uin: UIN,
                key_agreement: &agreement,
                tgtgt_key: tgtgt_key.as_bytes(),
            },
        )
        .is_err()
    );
    Ok(())
}

fn response_packet(
    state: u8,
    tlvs: &[u8],
    key: &QqTeaKey,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut plaintext = WireWriter::new(LIMIT);
    plaintext.put_u16(0x09)?;
    plaintext.put_u8(state)?;
    plaintext.put_bytes(tlvs)?;
    let encrypted = encrypt_qq_tea(&plaintext.finish(), key)?;

    let mut tail = WireWriter::new(LIMIT);
    tail.put_u16(8_001)?;
    tail.put_u16(2_064)?;
    tail.put_u16(0)?;
    tail.put_u32(UIN)?;
    tail.put_u8(0)?;
    tail.put_u16(0)?;
    tail.put_bytes(&encrypted)?;
    tail.put_u8(3)?;
    let tail = tail.finish();
    let declared = u16::try_from(tail.len() + 3)?;
    let mut packet = WireWriter::new(LIMIT);
    packet.put_u8(2)?;
    packet.put_u16(declared)?;
    packet.put_bytes(&tail)?;
    Ok(packet.finish())
}

fn collection(values: &[(u16, Vec<u8>)]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut writer = WireWriter::new(LIMIT);
    writer.put_u16(u16::try_from(values.len())?)?;
    for (tag, value) in values {
        writer.put_u16(*tag)?;
        writer.put_prefixed_bytes(LengthPrefix::U16Payload, value)?;
    }
    Ok(writer.finish())
}

fn profile_body(nickname: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut writer = WireWriter::new(LIMIT);
    writer.put_u16(7)?;
    writer.put_u8(18)?;
    writer.put_u8(2)?;
    writer.put_prefixed_bytes(LengthPrefix::U8Payload, nickname.as_bytes())?;
    Ok(writer.finish())
}

fn uid_body(uid: &str) -> Result<Vec<u8>, prost::EncodeError> {
    let value = UidEnvelope {
        layer_one: Some(UidLayerOne {
            layer_two: Some(UidLayerTwo {
                uid: uid.to_owned(),
            }),
        }),
    };
    let mut output = Vec::new();
    value.encode(&mut output)?;
    Ok(output)
}

fn notice_body(tag: &str, message: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut writer = WireWriter::new(LIMIT);
    writer.put_u32(2)?;
    writer.put_prefixed_bytes(LengthPrefix::U16Payload, tag.as_bytes())?;
    writer.put_prefixed_bytes(LengthPrefix::U16Payload, message.as_bytes())?;
    writer.put_u32(0)?;
    Ok(writer.finish())
}
