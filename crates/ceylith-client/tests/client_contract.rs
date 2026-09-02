//! Public client and server interoperability tests.

use ceylith_client::{
    AccessToken, Architecture, InstallationIdentity, OpaqueExchangeContext, PendingHandshake,
    Platform, ProfileVerifier, RequestedAccess, RuntimeDescriptor, decode_opaque_exchange_response,
};
use ceylith_crypto::{SecureSession, ServerHandshake, TRANSPORT_TAG_LEN};
use ceylith_protocol::{
    AccountSlotId, CURRENT_INNER_CONTRACT, CodecError, Digest32, ExchangeId, HandshakeEnvelope,
    HandshakeStep, OpaqueSlot, OpaqueSlotId, OpaqueSlots, ProfileId, ProfileOutcome, RequestId,
    SecureFrame, SessionId, WireLimits, decode_handshake_envelope, decode_inner_frame,
    encode_handshake_envelope, encode_inner_frame, encode_secure_frame, encode_secure_frame_header,
    profile_decision_signing_transcript, proto, session_hello_signing_transcript,
    validate_session_hello,
};
use ed25519_dalek::{Signer, SigningKey, Verifier, VerifyingKey};
use prost::Message;
use sha2::{Digest, Sha256};

fn runtime() -> Result<RuntimeDescriptor, Box<dyn std::error::Error>> {
    Ok(RuntimeDescriptor::new(
        1,
        2,
        vec![1],
        vec![1],
        Platform::Linux,
        Architecture::X86_64,
        Digest32::from_bytes([5; 32]),
    )?)
}

fn server_accept(
    encoded_hello: &[u8],
    server: &ServerHandshake,
    limits: WireLimits,
) -> Result<(Vec<u8>, SecureSession, SessionId), Box<dyn std::error::Error>> {
    let envelope = decode_handshake_envelope(encoded_hello, limits)?;
    assert_eq!(envelope.step(), HandshakeStep::ClientHello);
    let (pending, payload) = server.begin(envelope.payload())?;
    let hello = proto::SessionHello::decode(payload.as_slice())?;
    validate_session_hello(&hello)?;
    assert_eq!(
        pending.remote_static_key().as_bytes(),
        hello.installation_noise_public_key.as_slice()
    );
    let public_key: [u8; 32] = hello.installation_sign_public_key.as_slice().try_into()?;
    let signature: [u8; 64] = hello.transcript_signature.as_slice().try_into()?;
    VerifyingKey::from_bytes(&public_key)?.verify(
        &session_hello_signing_transcript(&hello)?,
        &ed25519_dalek::Signature::from_bytes(&signature),
    )?;

    let session_id = SessionId::from_bytes([6; 16]);
    let welcome = proto::SessionWelcome {
        session_id: session_id.as_bytes().to_vec(),
        runtime_lease: vec![7; 32],
        lease_expires_at_ms: 2_000,
        grant_class: proto::GrantClass::Community as i32,
        max_full_accounts: 3,
        max_active_installations: 2,
        max_registered_installations: 4,
        server_time_ms: 1_000,
        policy_epoch: 9,
        accepted_contracts: vec![CURRENT_INNER_CONTRACT],
    };
    let mut payload = Vec::with_capacity(welcome.encoded_len());
    welcome.encode(&mut payload)?;
    let (response, session, _) = pending.finish(&payload)?;
    let encoded = encode_handshake_envelope(
        &HandshakeEnvelope::new(HandshakeStep::ServerWelcome, response, limits)?,
        limits,
    )?;
    Ok((encoded, session, session_id))
}

fn server_open(
    session: &mut SecureSession,
    encoded: &[u8],
    limits: WireLimits,
) -> Result<(RequestId, proto::InnerFrame), Box<dyn std::error::Error>> {
    let frame = ceylith_protocol::decode_secure_frame(encoded, limits)?;
    let header = encode_secure_frame_header(
        frame.session_id(),
        frame.counter(),
        frame.request_id(),
        frame.ciphertext().len(),
        limits,
    )?;
    let plaintext = session.open(frame.counter(), &header, frame.ciphertext())?;
    Ok((frame.request_id(), decode_inner_frame(&plaintext, limits)?))
}

fn server_seal(
    session: &mut SecureSession,
    session_id: SessionId,
    request_id: RequestId,
    inner: &proto::InnerFrame,
    limits: WireLimits,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let plaintext = encode_inner_frame(inner, limits)?;
    let counter = session.next_send_counter();
    let header = encode_secure_frame_header(
        session_id,
        counter,
        request_id,
        plaintext.len() + TRANSPORT_TAG_LEN,
        limits,
    )?;
    let ciphertext = session.seal(counter, &header, &plaintext)?;
    Ok(encode_secure_frame(
        &SecureFrame::new(session_id, counter, request_id, ciphertext, limits)?,
        limits,
    )?)
}

#[test]
fn full_client_contract_interoperates_in_memory() -> Result<(), Box<dyn std::error::Error>> {
    let limits = WireLimits::default();
    let identity = InstallationIdentity::from_parts(
        ceylith_protocol::InstallationId::from_bytes([1; 16]),
        [2; 32],
        [3; 32],
    );
    let token = AccessToken::new(b"secret-token".to_vec())?;
    let server = ServerHandshake::new(ceylith_crypto::NoisePrivateKey::from_bytes([4; 32]));
    let (pending, encoded_hello) = PendingHandshake::start(
        &identity,
        server.public_key(),
        Some(&token),
        &runtime()?,
        0,
        limits,
    )?;
    assert!(!format!("{pending:?}").contains("secret-token"));
    let (encoded_welcome, mut server_session, session_id) =
        server_accept(&encoded_hello, &server, limits)?;
    let mut client = pending.finish(&encoded_welcome)?;
    assert_eq!(client.admission().session_id(), session_id);
    let watch = client.watch_request(7, 1_000)?;
    let Some(proto::inner_frame::Body::WatchRequest(watch)) = watch.body else {
        return Err(CodecError::InvalidField.into());
    };
    assert_eq!(watch.runtime_lease, vec![7; 32]);
    assert_eq!(watch.cursor, 7);
    assert_eq!(watch.max_wait_ms, 1_000);
    assert!(client.watch_request(7, 0).is_err());

    let request_id = RequestId::from_bytes([8; 16]);
    let request = client.profile_request(
        ProfileId::from_bytes([9; 16]),
        None,
        RequestedAccess::Full,
        &runtime()?,
    );
    let encoded_request = client.seal(request_id, &request)?;
    let (received_id, received) = server_open(&mut server_session, &encoded_request, limits)?;
    assert_eq!(received_id, request_id);
    assert_eq!(received, request);

    let response = proto::InnerFrame {
        contract: CURRENT_INNER_CONTRACT,
        body: Some(proto::inner_frame::Body::GenericResult(
            proto::GenericResult {
                accepted: true,
                code: 0,
                payload: vec![1, 2, 3],
            },
        )),
    };
    let encoded_response = server_seal(
        &mut server_session,
        session_id,
        request_id,
        &response,
        limits,
    )?;
    assert_eq!(client.open(&encoded_response)?, (request_id, response));

    let request_slots =
        OpaqueSlots::new(vec![OpaqueSlot::new(OpaqueSlotId::new(7)?, vec![8, 9])?])?;
    let exchange_context = OpaqueExchangeContext {
        exchange_id: ExchangeId::from_bytes([10; 16]),
        account_slot_id: AccountSlotId::from_bytes([11; 16]),
        generation: 2,
        expires_at_ms: 1_900,
        binding_digest: ceylith_protocol::opaque_binding_digest(
            ExchangeId::from_bytes([10; 16]),
            AccountSlotId::from_bytes([11; 16]),
            2,
            1_900,
            &request_slots,
        ),
    };
    let exchange_request =
        client.opaque_exchange_request(exchange_context, &request_slots, 1_100)?;
    let exchange_request_id = RequestId::from_bytes([13; 16]);
    let encoded_exchange = client.seal(exchange_request_id, &exchange_request)?;
    let (received_exchange_id, received_exchange) =
        server_open(&mut server_session, &encoded_exchange, limits)?;
    assert_eq!(received_exchange_id, exchange_request_id);
    assert_eq!(received_exchange, exchange_request);

    let exchange_response = proto::InnerFrame {
        contract: CURRENT_INNER_CONTRACT,
        body: Some(proto::inner_frame::Body::OpaqueExchangeResponse(
            proto::OpaqueExchangeResponse {
                exchange_id: exchange_context.exchange_id.as_bytes().to_vec(),
                generation: exchange_context.generation,
                slots: request_slots.to_wire(),
                expires_at_ms: 1_800,
                binding_digest: exchange_context.binding_digest.as_bytes().to_vec(),
            },
        )),
    };
    let encoded_exchange_response = server_seal(
        &mut server_session,
        session_id,
        exchange_request_id,
        &exchange_response,
        limits,
    )?;
    let (opened_id, opened_exchange) = client.open(&encoded_exchange_response)?;
    assert_eq!(opened_id, exchange_request_id);
    let result = decode_opaque_exchange_response(&opened_exchange, exchange_context, 1_200)?;
    assert_eq!(result.slots(), &request_slots);
    Ok(())
}

#[test]
fn profile_verifier_checks_expiry_digest_and_signature() -> Result<(), Box<dyn std::error::Error>> {
    let signing_key = SigningKey::from_bytes(&[7; 32]);
    let manifest = b"bounded public profile".to_vec();
    let mut decision = proto::ProfileDecision {
        status: proto::ProfileStatus::Ready as i32,
        profile_id: vec![9; 16],
        required_runtime_abi: 0,
        manifest: manifest.clone(),
        manifest_digest: Sha256::digest(&manifest).to_vec(),
        manifest_signature: vec![0; 64],
        expires_at_ms: 2_000,
        policy_epoch: 1,
    };
    decision.manifest_signature = signing_key
        .sign(&profile_decision_signing_transcript(&decision)?)
        .to_bytes()
        .to_vec();
    let verifier = ProfileVerifier::from_bytes(&signing_key.verifying_key().to_bytes())?;
    assert!(matches!(
        verifier.verify(&decision, 1_000)?,
        ProfileOutcome::Ready(_)
    ));

    let mut tampered = decision.clone();
    tampered.manifest[0] ^= 1;
    assert!(verifier.verify(&tampered, 1_000).is_err());
    assert!(verifier.verify(&decision, 2_000).is_err());
    Ok(())
}

#[test]
fn token_debug_is_redacted() -> Result<(), Box<dyn std::error::Error>> {
    let token = AccessToken::new(b"private-token-value".to_vec())?;
    let rendered = format!("{token:?}");
    assert!(rendered.contains("REDACTED"));
    assert!(!rendered.contains("private-token-value"));
    Ok(())
}
