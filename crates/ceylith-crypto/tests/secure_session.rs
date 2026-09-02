//! Fixed-suite handshake and secure-session tests.

use ceylith_crypto::{
    ClientHandshake, NoisePrivateKey, SecureSession, SecureSessionError, ServerHandshake,
    TRANSPORT_TAG_LEN,
};
use ceylith_protocol::{RequestId, SessionId, WireLimits, encode_secure_frame_header};

fn paired_sessions() -> Result<(SecureSession, SecureSession), SecureSessionError> {
    let client_key = NoisePrivateKey::from_bytes([7; 32]);
    let server = ServerHandshake::new(NoisePrivateKey::from_bytes([9; 32]));
    let (client_pending, request) =
        ClientHandshake::start(&client_key, server.public_key(), b"hello")?;
    let (server_pending, request_payload) = server.begin(&request)?;
    assert_eq!(request_payload, b"hello");
    assert_eq!(server_pending.remote_static_key(), client_key.public_key());
    let (response, server_session, server_binding) = server_pending.finish(b"welcome")?;
    let (response_payload, client_session, client_binding) = client_pending.finish(&response)?;
    assert_eq!(response_payload, b"welcome");
    assert_eq!(client_binding, server_binding);
    Ok((client_session, server_session))
}

fn header(counter: u64, plaintext_len: usize) -> Result<[u8; 52], Box<dyn std::error::Error>> {
    Ok(encode_secure_frame_header(
        SessionId::from_bytes([1; 16]),
        counter,
        RequestId::from_bytes([2; 16]),
        plaintext_len + TRANSPORT_TAG_LEN,
        WireLimits::default(),
    )?)
}

#[test]
fn handshake_and_transport_interoperate_in_both_directions()
-> Result<(), Box<dyn std::error::Error>> {
    let (mut client, mut server) = paired_sessions()?;
    let associated = header(0, 7)?;
    let ciphertext = client.seal(0, &associated, b"request")?;
    assert_eq!(server.open(0, &associated, &ciphertext)?, b"request");

    let associated = header(0, 8)?;
    let ciphertext = server.seal(0, &associated, b"response")?;
    assert_eq!(client.open(0, &associated, &ciphertext)?, b"response");
    Ok(())
}

#[test]
fn authentication_failure_closes_the_session() -> Result<(), Box<dyn std::error::Error>> {
    let (mut client, mut server) = paired_sessions()?;
    let associated = header(0, 3)?;
    let mut ciphertext = client.seal(0, &associated, b"one")?;
    ciphertext[0] ^= 1;
    assert_eq!(
        server.open(0, &associated, &ciphertext).err(),
        Some(SecureSessionError::AuthenticationFailed)
    );
    assert!(server.is_closed());
    assert_eq!(
        server.open(0, &associated, &ciphertext).err(),
        Some(SecureSessionError::SessionClosed)
    );
    Ok(())
}

#[test]
fn replay_and_message_limit_fail_closed() -> Result<(), Box<dyn std::error::Error>> {
    let (mut client, _) = paired_sessions()?;
    let associated = header(0, 3)?;
    assert_eq!(
        client.seal(1, &associated, b"one").err(),
        Some(SecureSessionError::CounterMismatch)
    );
    assert!(client.is_closed());

    let (client, _) = paired_sessions()?;
    let mut client = client.with_message_limit(1)?;
    client.seal(0, &associated, b"one")?;
    let next = header(1, 3)?;
    assert_eq!(
        client.seal(1, &next, b"two").err(),
        Some(SecureSessionError::SessionExpired)
    );
    Ok(())
}

#[test]
fn debug_output_never_contains_key_material() {
    let key = NoisePrivateKey::from_bytes([0x41; 32]);
    let rendered = format!("{key:?}");
    assert!(rendered.contains("REDACTED"));
    assert!(!rendered.contains("AAAA"));
}
