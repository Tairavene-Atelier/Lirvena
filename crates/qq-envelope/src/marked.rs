use prost::Message;

use crate::EnvelopeError;

const COMPILED_CONTRACT: u32 = 77;
const REQUIRED_SLOTS: [u16; 3] = [1, 2, 3];
const MAX_IDENTITY_LEN: usize = 256;
const MAX_CORRELATION_LEN: usize = 128;

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
        correlation: Some(correlation.to_owned()),
        account_identity: Some(account_identity.to_owned()),
        marked: Some(MarkedFields {
            third: third.to_vec(),
            first: first.to_vec(),
            second: second.to_vec(),
        }),
    }
    .encode_to_vec())
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
    #[prost(string, optional, tag = "15")]
    correlation: Option<String>,
    #[prost(string, optional, tag = "16")]
    account_identity: Option<String>,
    #[prost(message, optional, tag = "24")]
    marked: Option<MarkedFields>,
}
