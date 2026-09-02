use crate::{CodecError, FrameKind, HARD_MAX_OUTER_FRAME_LEN, LengthKind};

use super::{
    CURRENT_WIRE_VERSION, HANDSHAKE_HEADER_LEN, HANDSHAKE_MAGIC, HandshakeEnvelope, HandshakeStep,
    WireLimits, decode_u32_len, enforce_limit, require_exact_len, require_header, validate_version,
};

/// Encodes one complete handshake envelope.
///
/// # Errors
///
/// Returns an error when a bound is exceeded or a length cannot be represented.
pub fn encode_handshake_envelope(
    envelope: &HandshakeEnvelope,
    limits: WireLimits,
) -> Result<Vec<u8>, CodecError> {
    let payload_len = envelope.payload().len();
    enforce_limit(
        LengthKind::HandshakePayload,
        payload_len,
        limits.max_handshake_payload_len(),
    )?;
    let total_len = HANDSHAKE_HEADER_LEN
        .checked_add(payload_len)
        .ok_or(CodecError::LengthOverflow)?;
    enforce_limit(LengthKind::OuterFrame, total_len, HARD_MAX_OUTER_FRAME_LEN)?;
    let encoded_len = u32::try_from(payload_len).map_err(|_| CodecError::LengthOverflow)?;

    let mut output = Vec::with_capacity(total_len);
    output.extend_from_slice(&HANDSHAKE_MAGIC);
    output.extend_from_slice(&CURRENT_WIRE_VERSION.to_be_bytes());
    output.push(envelope.step() as u8);
    output.push(0);
    output.extend_from_slice(&encoded_len.to_be_bytes());
    output.extend_from_slice(envelope.payload());
    Ok(output)
}

/// Decodes exactly one complete handshake envelope.
///
/// # Errors
///
/// Returns an error for malformed, unsupported, truncated, trailing, or oversized input.
pub fn decode_handshake_envelope(
    input: &[u8],
    limits: WireLimits,
) -> Result<HandshakeEnvelope, CodecError> {
    require_header(input, HANDSHAKE_HEADER_LEN)?;
    if input[..4] != HANDSHAKE_MAGIC {
        return Err(CodecError::InvalidMagic {
            frame: FrameKind::Handshake,
        });
    }
    validate_version(u16::from_be_bytes([input[4], input[5]]))?;
    let step = HandshakeStep::try_from(input[6])?;
    if input[7] != 0 {
        return Err(CodecError::InvalidFlags);
    }
    let declared = decode_u32_len(&input[8..12])?;
    enforce_limit(
        LengthKind::HandshakePayload,
        declared,
        limits.max_handshake_payload_len(),
    )?;
    let total_len = HANDSHAKE_HEADER_LEN
        .checked_add(declared)
        .ok_or(CodecError::LengthOverflow)?;
    require_exact_len(input, total_len)?;
    HandshakeEnvelope::new(step, input[HANDSHAKE_HEADER_LEN..].to_vec(), limits)
}
