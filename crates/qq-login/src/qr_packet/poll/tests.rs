use ceylith_protocol::{OpaqueSlots, ProfileId};
use qq_envelope::{QqTeaKey, encrypt_qq_tea};
use qq_profile::{LinuxNtProfile, LinuxNtProfileSpec};
use qq_wire::{LengthPrefix, WireWriter};

use super::{QrPollContext, QrPollResponse, build_qr_poll, decode_qr_poll_response};
use crate::{QqKeyAgreement, QrChallenge, QrResponseContext};

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
fn poll_request_and_confirmed_response_preserve_required_values()
-> Result<(), Box<dyn std::error::Error>> {
    let profile = profile()?;
    let agreement = FakeAgreement(QqTeaKey::new([7; 16]));
    let random_key = QqTeaKey::new([8; 16]);
    let challenge = QrChallenge::for_test(vec![5; 24], "query-value".to_owned());
    let request = build_qr_poll(QrPollContext {
        profile: &profile,
        sso_sequence: 30,
        unix_seconds: 1_700_000_002,
        random_key: &random_key,
        key_agreement: &agreement,
        challenge: &challenge,
    })?;
    assert_eq!(request.command(), "wtlogin.trans_emp");
    assert_eq!(request.payload().len(), 196);

    let response = decode_qr_poll_response(
        &confirmed_packet(&agreement, 0)?,
        QrResponseContext {
            app_id: profile.app_id(),
            issued_at_ms: 2_000,
            random_key: &random_key,
            key_agreement: &agreement,
        },
    )?;
    let QrPollResponse::Confirmed(secrets) = response else {
        return Err("expected confirmed credentials".into());
    };
    assert_eq!(secrets.uin(), 10_001);
    assert_eq!(secrets.tgtgt_key(), [1; 16]);
    assert_eq!(secrets.temporary_password(), [2; 32]);
    assert_eq!(secrets.no_picture_signature(), [3; 24]);
    assert!(!format!("{secrets:?}").contains("[1"));
    Ok(())
}

#[test]
fn pending_response_accepts_bounded_generation_tail() -> Result<(), Box<dyn std::error::Error>> {
    let profile = profile()?;
    let agreement = FakeAgreement(QqTeaKey::new([7; 16]));
    let random_key = QqTeaKey::new([8; 16]);
    let response = decode_qr_poll_response(
        &state_packet(&agreement, 48, &[0xaa, 0xbb, 0xcc])?,
        QrResponseContext {
            app_id: profile.app_id(),
            issued_at_ms: 2_000,
            random_key: &random_key,
            key_agreement: &agreement,
        },
    )?;
    assert!(matches!(
        response,
        QrPollResponse::State(crate::QrPollState::WaitingForScan)
    ));
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

fn confirmed_packet(
    agreement: &FakeAgreement,
    sequence: u16,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut tlvs = WireWriter::new(4_096);
    tlvs.put_u16(3)?;
    write_tlv(&mut tlvs, 0x01e, &[1; 16])?;
    write_tlv(&mut tlvs, 0x018, &[2; 32])?;
    write_tlv(&mut tlvs, 0x019, &[3; 24])?;

    let mut response = WireWriter::new(8_192);
    response.put_u16(0)?;
    response.put_u32(1_001)?;
    response.put_u8(0)?;
    response.put_u64(10_001)?;
    response.put_u32(0)?;
    response.put_bytes(&tlvs.finish())?;
    wrap_response_with_sequence(agreement, &response.finish(), sequence)
}

fn state_packet(
    agreement: &FakeAgreement,
    state: u8,
    tail: &[u8],
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut response = WireWriter::new(8_192);
    response.put_u16(0)?;
    response.put_u32(1_001)?;
    response.put_u8(state)?;
    response.put_bytes(tail)?;
    wrap_response(agreement, &response.finish())
}

fn wrap_response(
    agreement: &FakeAgreement,
    response: &[u8],
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    wrap_response_with_sequence(agreement, response, 0)
}

fn wrap_response_with_sequence(
    agreement: &FakeAgreement,
    response: &[u8],
    sequence: u16,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut transaction = WireWriter::new(8_192);
    transaction.put_u8(2)?;
    transaction.put_u16(u16::try_from(response.len() + 44)?)?;
    transaction.put_u16(0x12)?;
    transaction.put_bytes(&[0; 21])?;
    transaction.put_u8(3)?;
    transaction.put_u16(0)?;
    transaction.put_u16(0x32)?;
    transaction.put_u32(0)?;
    transaction.put_u64(0)?;
    transaction.put_bytes(response)?;
    transaction.put_u8(3)?;
    let transaction = transaction.finish();

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

fn write_tlv(writer: &mut WireWriter, tag: u16, body: &[u8]) -> Result<(), qq_wire::WireError> {
    writer.put_u16(tag)?;
    writer.put_prefixed_bytes(LengthPrefix::U16Payload, body)
}
