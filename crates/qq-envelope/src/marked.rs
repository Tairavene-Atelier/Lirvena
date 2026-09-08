use prost::Message;

use crate::EnvelopeError;

const COMPILED_CONTRACT: u32 = 77;
const REQUIRED_SLOTS: [u16; 3] = [1, 2, 3];
const MAX_IDENTITY_LEN: usize = 256;
const MAX_CORRELATION_LEN: usize = 128;
const ACCOUNT_IDENTITY_FIELD: u32 = 16;

/// Borrowed numeric mark carried by one authenticated action directive.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EnvelopeMark<'a> {
    /// Compiled numeric insertion slot.
    pub slot: u16,
    /// Opaque bytes for that slot.
    pub value: &'a [u8],
}

/// Encodes the only compiled marked-reserve contract supported by this Lirvena build.
///
/// The caller supplies authenticated numeric marks; this function owns their QQ envelope
/// placement. Unknown contracts, duplicate slots and incomplete mark sets fail closed.
///
/// # Errors
///
/// Returns an error when the contract or bounded fields are not recognized.
pub fn encode_marked_reserve(
    contract: u32,
    marks: &[EnvelopeMark<'_>],
    correlation: &str,
    account_identity: &str,
    include_identity: bool,
) -> Result<Vec<u8>, EnvelopeError> {
    if contract != COMPILED_CONTRACT
        || marks.len() != REQUIRED_SLOTS.len()
        || correlation.is_empty()
        || correlation.len() > MAX_CORRELATION_LEN
        || account_identity.is_empty()
        || account_identity.len() > MAX_IDENTITY_LEN
    {
        return Err(EnvelopeError::InvalidField);
    }
    let mut values: [Option<&[u8]>; 3] = [None, None, None];
    for mark in marks {
        let index = REQUIRED_SLOTS
            .iter()
            .position(|slot| *slot == mark.slot)
            .ok_or(EnvelopeError::InvalidField)?;
        if values[index].replace(mark.value).is_some() {
            return Err(EnvelopeError::InvalidField);
        }
    }
    let [Some(first), Some(second), Some(third)] = values else {
        return Err(EnvelopeError::InvalidField);
    };
    if third.is_empty() {
        return Err(EnvelopeError::InvalidField);
    }
    Ok(ReserveFields {
        locale: include_identity.then_some(2_052),
        new_connection: include_identity.then_some(1),
        correlation: Some(correlation.to_owned()),
        account_identity: Some(account_identity.to_owned()),
        subscriber_identity: include_identity.then_some(0),
        network: include_identity.then_some(1),
        address_stack: include_identity.then_some(1),
        marked: Some(MarkedFields {
            third: third.to_vec(),
            first: first.to_vec(),
            second: second.to_vec(),
        }),
        core_version: include_identity.then_some(100),
    }
    .encode_to_vec())
}

/// Encodes the ordinary per-packet reserve used after QQ has supplied an account identity.
///
/// # Errors
///
/// Returns an error when the account identity is empty, oversized or entropy is unavailable.
pub fn encode_account_reserve(
    account_identity: &str,
    include_identity: bool,
) -> Result<Vec<u8>, EnvelopeError> {
    validate_account_identity(account_identity)?;
    Ok(ReserveFields {
        locale: include_identity.then_some(2_052),
        new_connection: include_identity.then_some(1),
        correlation: Some(random_correlation()?),
        account_identity: Some(account_identity.to_owned()),
        subscriber_identity: include_identity.then_some(0),
        network: include_identity.then_some(1),
        address_stack: include_identity.then_some(1),
        marked: None,
        core_version: include_identity.then_some(100),
    }
    .encode_to_vec())
}

/// Inserts the login account identity into a Ceylith-produced reserve without changing any
/// existing opaque protobuf fields.
///
/// # Errors
///
/// Returns an error for malformed protobuf, a duplicate identity field or invalid identity text.
pub fn attach_account_identity(
    reserve: &[u8],
    account_identity: &str,
) -> Result<Vec<u8>, EnvelopeError> {
    validate_account_identity(account_identity)?;
    let insertion = insertion_offset(reserve, ACCOUNT_IDENTITY_FIELD)?;
    let mut field = Vec::with_capacity(account_identity.len() + 4);
    put_varint((u64::from(ACCOUNT_IDENTITY_FIELD) << 3) | 2, &mut field);
    put_varint(
        u64::try_from(account_identity.len()).map_err(|_error| EnvelopeError::InvalidField)?,
        &mut field,
    );
    field.extend_from_slice(account_identity.as_bytes());
    let mut completed = Vec::with_capacity(reserve.len() + field.len());
    completed.extend_from_slice(&reserve[..insertion]);
    completed.extend_from_slice(&field);
    completed.extend_from_slice(&reserve[insertion..]);
    Ok(completed)
}

fn validate_account_identity(value: &str) -> Result<(), EnvelopeError> {
    if value.is_empty() || value.len() > MAX_IDENTITY_LEN || value.chars().any(char::is_control) {
        Err(EnvelopeError::InvalidField)
    } else {
        Ok(())
    }
}

fn random_correlation() -> Result<String, EnvelopeError> {
    let mut entropy = [0_u8; 24];
    getrandom::fill(&mut entropy).map_err(|_error| EnvelopeError::InvalidField)?;
    let mut value = String::with_capacity(55);
    value.push_str("00-");
    append_hex(&mut value, &entropy[..16]);
    value.push('-');
    append_hex(&mut value, &entropy[16..]);
    value.push_str("-01");
    Ok(value)
}

fn append_hex(output: &mut String, bytes: &[u8]) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
}

fn insertion_offset(encoded: &[u8], target: u32) -> Result<usize, EnvelopeError> {
    let mut offset = 0;
    let mut insertion = encoded.len();
    while offset < encoded.len() {
        let field_start = offset;
        let key = read_varint(encoded, &mut offset)?;
        let number = u32::try_from(key >> 3).map_err(|_error| EnvelopeError::InvalidField)?;
        if number == 0 || number == target {
            return Err(EnvelopeError::InvalidField);
        }
        if number > target && insertion == encoded.len() {
            insertion = field_start;
        }
        skip_field(encoded, &mut offset, key & 7)?;
    }
    Ok(insertion)
}

fn skip_field(encoded: &[u8], offset: &mut usize, wire_type: u64) -> Result<(), EnvelopeError> {
    let length = match wire_type {
        0 => {
            read_varint(encoded, offset)?;
            return Ok(());
        }
        1 => 8,
        2 => usize::try_from(read_varint(encoded, offset)?)
            .map_err(|_error| EnvelopeError::InvalidField)?,
        5 => 4,
        _ => return Err(EnvelopeError::InvalidField),
    };
    *offset = offset
        .checked_add(length)
        .filter(|end| *end <= encoded.len())
        .ok_or(EnvelopeError::InvalidField)?;
    Ok(())
}

fn read_varint(encoded: &[u8], offset: &mut usize) -> Result<u64, EnvelopeError> {
    let mut value = 0_u64;
    for shift in (0..70).step_by(7) {
        let byte = *encoded.get(*offset).ok_or(EnvelopeError::InvalidField)?;
        *offset += 1;
        if shift == 63 && byte > 1 {
            return Err(EnvelopeError::InvalidField);
        }
        value |= u64::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return Ok(value);
        }
    }
    Err(EnvelopeError::InvalidField)
}

fn put_varint(mut value: u64, output: &mut Vec<u8>) {
    while value >= 0x80 {
        let low = value.to_le_bytes()[0] & 0x7f;
        output.push(low | 0x80);
        value >>= 7;
    }
    output.push(value.to_le_bytes()[0]);
}

#[derive(Clone, PartialEq, Message)]
struct MarkedFields {
    #[prost(bytes = "vec", tag = "1")]
    third: Vec<u8>,
    #[prost(bytes = "vec", tag = "2")]
    first: Vec<u8>,
    #[prost(bytes = "vec", tag = "3")]
    second: Vec<u8>,
}

#[derive(Clone, PartialEq, Message)]
struct ReserveFields {
    #[prost(uint32, optional, tag = "11")]
    locale: Option<u32>,
    #[prost(uint32, optional, tag = "14")]
    new_connection: Option<u32>,
    #[prost(string, optional, tag = "15")]
    correlation: Option<String>,
    #[prost(string, optional, tag = "16")]
    account_identity: Option<String>,
    #[prost(uint32, optional, tag = "18")]
    subscriber_identity: Option<u32>,
    #[prost(uint32, optional, tag = "19")]
    network: Option<u32>,
    #[prost(uint32, optional, tag = "20")]
    address_stack: Option<u32>,
    #[prost(message, optional, tag = "24")]
    marked: Option<MarkedFields>,
    #[prost(uint32, optional, tag = "26")]
    core_version: Option<u32>,
}
