//! Unified anonymous and authenticated session-envelope tests.

use qq_envelope::{
    ExpectedSsoResponse, QqTeaKey, SessionAuth, SessionRequestParts, decode_session_frame,
    decode_session_response, decrypt_qq_tea, encode_session_request, encrypt_qq_tea,
};
use qq_wire::{LengthPrefix, WireReader, WireWriter};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

const COMMAND: &str = "example.route";
const GUID: &[u8; 32] = b"00112233445566778899AABBCCDDEEFF";

#[test]
fn authenticated_request_uses_one_validated_session_path() -> TestResult {
    let key = QqTeaKey::new([7; 16]);
    let auth = SessionAuth::authenticated(42, b"tgt", b"d2", &key)?;
    let encoded = encode_session_request(request(&auth))?;
    let mut outer = WireReader::new(&encoded);
    let body = outer.read_prefixed_bytes(LengthPrefix::U32Inclusive, 4_096)?;
    outer.finish()?;
    let mut body = WireReader::new(body);
    assert_eq!(body.read_u32()?, 12);
    assert_eq!(body.read_u8()?, 1);
    assert_eq!(
        body.read_prefixed_bytes(LengthPrefix::U32Inclusive, 32)?,
        b"d2"
    );
    assert_eq!(body.read_u8()?, 0);
    assert_eq!(
        body.read_prefixed_bytes(LengthPrefix::U32Inclusive, 32)?,
        b"42"
    );
    assert!(!decrypt_qq_tea(body.read_bytes(body.remaining())?, &key)?.is_empty());
    assert!(!format!("{auth:?}").contains("tgt"));
    Ok(())
}

#[test]
fn response_binding_rejects_wrong_account_sequence_or_command() -> TestResult {
    let key = QqTeaKey::new([9; 16]);
    let auth = SessionAuth::authenticated(42, b"tgt", b"d2", &key)?;
    let response = response_frame(42, &key, 17, COMMAND)?;
    let decoded = decode_session_response(
        &response,
        ExpectedSsoResponse {
            auth: &auth,
            sequence: 17,
            command: COMMAND,
        },
    )?;
    assert_eq!(decoded.return_code(), 0);
    let uncorrelated = response_frame(42, &key, 99, "push.alpha")?;
    assert_eq!(
        decode_session_frame(&uncorrelated, &auth)?.command(),
        "push.alpha"
    );
    assert!(
        decode_session_response(
            &response,
            ExpectedSsoResponse {
                auth: &auth,
                sequence: 18,
                command: COMMAND,
            },
        )
        .is_err()
    );
    Ok(())
}

#[test]
fn incomplete_authenticated_state_is_rejected() {
    let key = QqTeaKey::new([1; 16]);
    assert!(SessionAuth::authenticated(0, b"tgt", b"d2", &key).is_err());
    assert!(SessionAuth::authenticated(42, b"", b"d2", &key).is_err());
    assert!(SessionAuth::authenticated(42, b"tgt", b"", &key).is_err());
}

fn request<'a>(auth: &'a SessionAuth<'a>) -> SessionRequestParts<'a> {
    SessionRequestParts {
        auth,
        sequence: 17,
        sub_app_id: 2,
        locale_id: 2_052,
        command: COMMAND,
        device_guid_hex: GUID,
        client_version: "1.2.3-456",
        reserve: b"reserve",
        payload: b"payload",
    }
}

fn response_frame(uin: u32, key: &QqTeaKey, sequence: u32, command: &str) -> TestResult<Vec<u8>> {
    let mut header = WireWriter::new(4_096);
    header.put_u32(sequence)?;
    header.put_u32(0)?;
    header.put_prefixed_bytes(LengthPrefix::U32Inclusive, b"")?;
    header.put_prefixed_bytes(LengthPrefix::U32Inclusive, command.as_bytes())?;
    header.put_prefixed_bytes(LengthPrefix::U32Inclusive, b"")?;
    header.put_u32(0)?;
    header.put_prefixed_bytes(LengthPrefix::U32Inclusive, b"")?;
    let mut sso = WireWriter::new(4_096);
    sso.put_prefixed_bytes(LengthPrefix::U32Inclusive, &header.finish())?;
    sso.put_prefixed_bytes(LengthPrefix::U32Inclusive, b"ok")?;
    let encrypted = encrypt_qq_tea(&sso.finish(), key)?;

    let mut body = WireWriter::new(4_096);
    body.put_u32(12)?;
    body.put_u8(1)?;
    body.put_u8(0)?;
    body.put_prefixed_bytes(LengthPrefix::U32Inclusive, uin.to_string().as_bytes())?;
    body.put_bytes(&encrypted)?;
    let mut frame = WireWriter::new(4_096);
    frame.put_prefixed_bytes(LengthPrefix::U32Inclusive, &body.finish())?;
    Ok(frame.finish())
}
