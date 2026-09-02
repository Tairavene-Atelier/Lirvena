pub(crate) mod validate;
mod wire_scan;

use prost::Message;

use crate::{CodecError, LengthKind, WireLimits, proto};

/// Current encrypted inner-contract version.
pub const CURRENT_INNER_CONTRACT: u32 = 4;

/// Encodes one structurally validated inner frame.
///
/// # Errors
///
/// Returns an error when the frame is invalid, oversized, or cannot be encoded.
pub fn encode_inner_frame(
    frame: &proto::InnerFrame,
    limits: WireLimits,
) -> Result<Vec<u8>, CodecError> {
    validate::validate_inner(frame)?;
    let encoded_len = frame.encoded_len();
    enforce_inner_limit(encoded_len, limits)?;
    let mut encoded = Vec::with_capacity(encoded_len);
    frame
        .encode(&mut encoded)
        .map_err(|_| CodecError::Protobuf)?;
    Ok(encoded)
}

/// Decodes exactly one structurally validated inner frame.
///
/// # Errors
///
/// Returns an error when the input is malformed, ambiguous, or exceeds its bound.
pub fn decode_inner_frame(
    input: &[u8],
    limits: WireLimits,
) -> Result<proto::InnerFrame, CodecError> {
    enforce_inner_limit(input.len(), limits)?;
    wire_scan::validate_top_level(input)?;
    let frame = proto::InnerFrame::decode(input).map_err(|_| CodecError::Protobuf)?;
    validate::validate_inner(&frame)?;
    Ok(frame)
}

fn enforce_inner_limit(actual: usize, limits: WireLimits) -> Result<(), CodecError> {
    let limit = limits.max_inner_frame_len();
    if actual > limit {
        Err(CodecError::LengthLimitExceeded {
            kind: LengthKind::InnerFrame,
            limit,
            actual,
        })
    } else {
        Ok(())
    }
}
