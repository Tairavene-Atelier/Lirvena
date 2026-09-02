use crate::{CodecError, inner::CURRENT_INNER_CONTRACT};

const MIN_BODY_TAG: u64 = 10;
const MAX_BODY_TAG: u64 = 63;

pub(super) fn validate_top_level(input: &[u8]) -> Result<(), CodecError> {
    let mut position = 0;
    let mut contract_seen = false;
    let mut body_seen = false;

    while position < input.len() {
        let key = read_varint(input, &mut position)?;
        let tag = key >> 3;
        let wire_type = (key & 7) as u8;
        if tag == 0 {
            return Err(CodecError::Protobuf);
        }

        if tag == 1 {
            if contract_seen || wire_type != 0 {
                return Err(CodecError::InvalidContract);
            }
            let contract = read_varint(input, &mut position)?;
            if contract != u64::from(CURRENT_INNER_CONTRACT) {
                return Err(CodecError::InvalidContract);
            }
            contract_seen = true;
            continue;
        }

        if (MIN_BODY_TAG..=MAX_BODY_TAG).contains(&tag) {
            if !known_body_tag(tag) || body_seen || wire_type != 2 {
                return Err(CodecError::UnsupportedBody);
            }
            body_seen = true;
        }
        skip_value(input, &mut position, wire_type)?;
    }

    if !contract_seen {
        return Err(CodecError::InvalidContract);
    }
    if !body_seen {
        return Err(CodecError::UnsupportedBody);
    }
    Ok(())
}

const fn known_body_tag(tag: u64) -> bool {
    matches!(
        tag,
        10 | 11 | 12 | 13 | 14 | 15 | 16 | 17 | 18 | 19 | 20 | 21 | 63
    )
}

fn read_varint(input: &[u8], position: &mut usize) -> Result<u64, CodecError> {
    let mut value = 0_u64;
    for shift in (0..70).step_by(7) {
        let byte = *input.get(*position).ok_or(CodecError::Truncated {
            needed: position.saturating_add(1),
            available: input.len(),
        })?;
        *position = position.checked_add(1).ok_or(CodecError::LengthOverflow)?;

        if shift == 63 && byte > 1 {
            return Err(CodecError::Protobuf);
        }
        value |= u64::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return Ok(value);
        }
    }
    Err(CodecError::Protobuf)
}

fn skip_value(input: &[u8], position: &mut usize, wire_type: u8) -> Result<(), CodecError> {
    let width = match wire_type {
        0 => {
            read_varint(input, position)?;
            return Ok(());
        }
        1 => 8,
        2 => usize::try_from(read_varint(input, position)?)
            .map_err(|_| CodecError::LengthOverflow)?,
        5 => 4,
        _ => return Err(CodecError::Protobuf),
    };
    let end = position
        .checked_add(width)
        .ok_or(CodecError::LengthOverflow)?;
    if end > input.len() {
        return Err(CodecError::Truncated {
            needed: end,
            available: input.len(),
        });
    }
    *position = end;
    Ok(())
}
