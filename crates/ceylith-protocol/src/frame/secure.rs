use crate::{CodecError, FrameKind, HARD_MAX_OUTER_FRAME_LEN, LengthKind, RequestId, SessionId};

use super::{
    CURRENT_WIRE_VERSION, SECURE_FRAME_HEADER_LEN, SECURE_FRAME_MAGIC, SecureFrame, WireLimits,
    decode_u32_len, enforce_limit, require_exact_len, require_header, validate_version,
};

/// Encodes the fixed 52-byte header used as authenticated associated data.
///
/// # Errors
///
/// Returns an error when a bound is exceeded or a length cannot be represented.
pub fn encode_secure_frame_header(
    session_id: SessionId,
    counter: u64,
    request_id: RequestId,
    ciphertext_len: usize,
    limits: WireLimits,
) -> Result<[u8; SECURE_FRAME_HEADER_LEN], CodecError> {
    enforce_limit(
        LengthKind::Ciphertext,
        ciphertext_len,
        limits.max_ciphertext_len(),
    )?;
    let total_len = SECURE_FRAME_HEADER_LEN
        .checked_add(ciphertext_len)
        .ok_or(CodecError::LengthOverflow)?;
    enforce_limit(LengthKind::OuterFrame, total_len, HARD_MAX_OUTER_FRAME_LEN)?;
    let encoded_len = u32::try_from(ciphertext_len).map_err(|_| CodecError::LengthOverflow)?;

    let mut header = [0_u8; SECURE_FRAME_HEADER_LEN];
    header[..4].copy_from_slice(&SECURE_FRAME_MAGIC);
    header[4..6].copy_from_slice(&CURRENT_WIRE_VERSION.to_be_bytes());
    header[6..8].copy_from_slice(&0_u16.to_be_bytes());
    header[8..24].copy_from_slice(session_id.as_bytes());
    header[24..32].copy_from_slice(&counter.to_be_bytes());
    header[32..48].copy_from_slice(request_id.as_bytes());
    header[48..52].copy_from_slice(&encoded_len.to_be_bytes());
    Ok(header)
}

/// Encodes one complete encrypted frame.
///
/// # Errors
///
/// Returns an error when a bound is exceeded or a length cannot be represented.
pub fn encode_secure_frame(frame: &SecureFrame, limits: WireLimits) -> Result<Vec<u8>, CodecError> {
    let header = encode_secure_frame_header(
        frame.session_id(),
        frame.counter(),
        frame.request_id(),
        frame.ciphertext().len(),
        limits,
    )?;
    let total_len = header
        .len()
        .checked_add(frame.ciphertext().len())
        .ok_or(CodecError::LengthOverflow)?;
    let mut output = Vec::with_capacity(total_len);
    output.extend_from_slice(&header);
    output.extend_from_slice(frame.ciphertext());
    Ok(output)
}

/// Decodes exactly one complete encrypted frame.
///
/// # Errors
///
/// Returns an error for malformed, unsupported, truncated, trailing, or oversized input.
pub fn decode_secure_frame(input: &[u8], limits: WireLimits) -> Result<SecureFrame, CodecError> {
    require_header(input, SECURE_FRAME_HEADER_LEN)?;
    if input[..4] != SECURE_FRAME_MAGIC {
        return Err(CodecError::InvalidMagic {
            frame: FrameKind::Secure,
        });
    }
    validate_version(u16::from_be_bytes([input[4], input[5]]))?;
    if input[6..8] != [0, 0] {
        return Err(CodecError::InvalidFlags);
    }
    let session_id = SessionId::try_from(&input[8..24]).map_err(|_| CodecError::InvalidField)?;
    let counter =
        u64::from_be_bytes(
            input[24..32]
                .try_into()
                .map_err(|_| CodecError::Truncated {
                    needed: SECURE_FRAME_HEADER_LEN,
                    available: input.len(),
                })?,
        );
    let request_id = RequestId::try_from(&input[32..48]).map_err(|_| CodecError::InvalidField)?;
    let declared = decode_u32_len(&input[48..52])?;
    enforce_limit(
        LengthKind::Ciphertext,
        declared,
        limits.max_ciphertext_len(),
    )?;
    let total_len = SECURE_FRAME_HEADER_LEN
        .checked_add(declared)
        .ok_or(CodecError::LengthOverflow)?;
    require_exact_len(input, total_len)?;
    SecureFrame::new(
        session_id,
        counter,
        request_id,
        input[SECURE_FRAME_HEADER_LEN..].to_vec(),
        limits,
    )
}
