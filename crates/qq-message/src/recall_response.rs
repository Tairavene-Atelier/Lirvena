use prost::Message;

use crate::MessageDecodeError;

const MAX_RECALL_RESPONSE_BYTES: usize = 64 * 1024;

/// Validates one Linux NT group-recall acknowledgement against the requested message sequence.
///
/// # Errors
///
/// Returns an error for a malformed, oversized, unsuccessful, ambiguous, or mismatched response.
pub fn validate_group_recall_response(
    input: &[u8],
    expected_sequence: u64,
) -> Result<(), MessageDecodeError> {
    validate_input(input, expected_sequence)?;
    let response = GroupRecallResponse::decode(input).map_err(|_error| MessageDecodeError)?;
    if response.result != 0 || response.items.len() != 1 {
        return Err(MessageDecodeError);
    }
    let item = &response.items[0];
    if item.result != 0
        || item
            .identity
            .as_ref()
            .is_none_or(|identity| identity.sequence != expected_sequence)
    {
        return Err(MessageDecodeError);
    }
    Ok(())
}

/// Validates one Linux NT direct-message recall acknowledgement against the requested client
/// sequence.
///
/// # Errors
///
/// Returns an error for a malformed, oversized, unsuccessful, ambiguous, or mismatched response.
pub fn validate_private_recall_response(
    input: &[u8],
    expected_client_sequence: u64,
) -> Result<(), MessageDecodeError> {
    validate_input(input, expected_client_sequence)?;
    let response = PrivateRecallResponse::decode(input).map_err(|_error| MessageDecodeError)?;
    if response.result != 0 || response.items.len() != 1 {
        return Err(MessageDecodeError);
    }
    let item = &response.items[0];
    if item.result != 0
        || item
            .identity
            .as_ref()
            .is_none_or(|identity| identity.client_sequence != expected_client_sequence)
    {
        return Err(MessageDecodeError);
    }
    Ok(())
}

fn validate_input(input: &[u8], expected_correlation: u64) -> Result<(), MessageDecodeError> {
    if input.is_empty() || input.len() > MAX_RECALL_RESPONSE_BYTES || expected_correlation == 0 {
        return Err(MessageDecodeError);
    }
    Ok(())
}

#[derive(Clone, PartialEq, Message)]
struct GroupRecallResponse {
    #[prost(int32, tag = "1")]
    result: i32,
    #[prost(string, tag = "2")]
    error_message: String,
    #[prost(message, repeated, tag = "4")]
    items: Vec<GroupRecallResponseItem>,
}

#[derive(Clone, PartialEq, Message)]
struct GroupRecallResponseItem {
    #[prost(message, optional, tag = "1")]
    identity: Option<GroupRecallResponseIdentity>,
    #[prost(int32, tag = "2")]
    result: i32,
    #[prost(string, tag = "3")]
    error_message: String,
}

#[derive(Clone, Copy, PartialEq, Message)]
struct GroupRecallResponseIdentity {
    #[prost(uint64, tag = "1")]
    sequence: u64,
}

#[derive(Clone, PartialEq, Message)]
struct PrivateRecallResponse {
    #[prost(int32, tag = "1")]
    result: i32,
    #[prost(string, tag = "2")]
    error_message: String,
    #[prost(message, repeated, tag = "5")]
    items: Vec<PrivateRecallResponseItem>,
}

#[derive(Clone, PartialEq, Message)]
struct PrivateRecallResponseItem {
    #[prost(int32, tag = "1")]
    result: i32,
    #[prost(string, tag = "2")]
    error_message: String,
    #[prost(message, optional, tag = "3")]
    identity: Option<PrivateRecallResponseIdentity>,
}

#[derive(Clone, Copy, PartialEq, Message)]
struct PrivateRecallResponseIdentity {
    #[prost(uint64, tag = "6")]
    client_sequence: u64,
}
