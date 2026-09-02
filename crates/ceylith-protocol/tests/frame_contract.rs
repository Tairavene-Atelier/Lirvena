//! Fixed outer-frame contract tests.

use ceylith_protocol::{
    CodecError, FrameKind, HandshakeEnvelope, HandshakeStep, RequestId, SECURE_FRAME_HEADER_LEN,
    SecureFrame, SessionId, WireLimits, decode_handshake_envelope, decode_secure_frame,
    encode_handshake_envelope, encode_secure_frame, encode_secure_frame_header,
};

#[test]
fn handshake_envelope_round_trips_exactly() -> Result<(), CodecError> {
    let limits = WireLimits::default();
    let envelope = HandshakeEnvelope::new(HandshakeStep::ClientHello, vec![1, 2, 3], limits)?;
    let encoded = encode_handshake_envelope(&envelope, limits)?;

    assert_eq!(decode_handshake_envelope(&encoded, limits)?, envelope);
    assert_eq!(
        decode_handshake_envelope(&encoded[..11], limits).err(),
        Some(CodecError::Truncated {
            needed: 12,
            available: 11
        })
    );

    let mut trailing = encoded.clone();
    trailing.push(0);
    assert!(matches!(
        decode_handshake_envelope(&trailing, limits),
        Err(CodecError::TrailingBytes { .. })
    ));

    let mut bad_magic = encoded;
    bad_magic[0] ^= 1;
    assert_eq!(
        decode_handshake_envelope(&bad_magic, limits).err(),
        Some(CodecError::InvalidMagic {
            frame: FrameKind::Handshake
        })
    );
    Ok(())
}

#[test]
fn secure_frame_header_is_canonical_and_round_trips() -> Result<(), CodecError> {
    let limits = WireLimits::default();
    let session_id = SessionId::from_bytes([7; 16]);
    let request_id = RequestId::from_bytes([9; 16]);
    let ciphertext = vec![4; 32];
    let frame = SecureFrame::new(session_id, 42, request_id, ciphertext, limits)?;
    let encoded = encode_secure_frame(&frame, limits)?;
    let header = encode_secure_frame_header(session_id, 42, request_id, 32, limits)?;

    assert_eq!(encoded[..SECURE_FRAME_HEADER_LEN], header);
    assert_eq!(decode_secure_frame(&encoded, limits)?, frame);

    let mut flags = encoded;
    flags[7] = 1;
    assert_eq!(
        decode_secure_frame(&flags, limits).err(),
        Some(CodecError::InvalidFlags)
    );
    Ok(())
}

#[test]
fn configured_limits_only_tighten_hard_bounds() -> Result<(), CodecError> {
    let limits = WireLimits::new(2, 16, 32)?;
    assert!(matches!(
        HandshakeEnvelope::new(HandshakeStep::ClientHello, vec![0; 3], limits),
        Err(CodecError::LengthLimitExceeded { .. })
    ));
    assert!(matches!(
        SecureFrame::new(
            SessionId::from_bytes([1; 16]),
            0,
            RequestId::from_bytes([2; 16]),
            vec![0; 17],
            limits
        ),
        Err(CodecError::LengthLimitExceeded { .. })
    ));
    Ok(())
}
