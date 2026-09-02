use prost::Message;

use crate::MessageDecodeError;

const MAX_UID_BYTES: usize = 128;

/// Input for one group-message recall request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GroupRecallInput {
    /// Numeric group identifier.
    pub group_uin: u64,
    /// QQ message sequence within the group.
    pub sequence: u64,
}

/// Input for one direct-message recall request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PrivateRecallInput<'a> {
    /// Peer UID resolved for the current QQ generation.
    pub target_uid: &'a str,
    /// QQ server message sequence.
    pub sequence: u64,
    /// Client sequence used by the original send operation.
    pub client_sequence: u64,
    /// Message random used by the original send operation.
    pub random: u32,
    /// QQ server timestamp.
    pub timestamp: u32,
}

/// Encodes the compiled Linux NT group recall request.
///
/// # Errors
///
/// Returns an error for a zero group or message sequence.
pub fn encode_group_recall(input: GroupRecallInput) -> Result<Vec<u8>, MessageDecodeError> {
    if input.group_uin == 0 || input.sequence == 0 {
        return Err(MessageDecodeError);
    }
    Ok(GroupRecallRequest {
        kind: 1,
        group_uin: input.group_uin,
        message: Some(GroupRecallMessage {
            sequence: input.sequence,
            random: 0,
            reserved: 0,
        }),
        settings: Some(GroupRecallSettings { reserved: 0 }),
    }
    .encode_to_vec())
}

/// Encodes the compiled Linux NT direct-message recall request.
///
/// # Errors
///
/// Returns an error for missing correlations or an invalid peer UID.
pub fn encode_private_recall(input: PrivateRecallInput<'_>) -> Result<Vec<u8>, MessageDecodeError> {
    if input.target_uid.is_empty()
        || input.target_uid.len() > MAX_UID_BYTES
        || input.target_uid.chars().any(char::is_control)
        || input.sequence == 0
        || input.client_sequence == 0
        || input.random == 0
        || input.timestamp == 0
    {
        return Err(MessageDecodeError);
    }
    Ok(PrivateRecallRequest {
        kind: 1,
        target_uid: input.target_uid.to_owned(),
        message: Some(PrivateRecallMessage {
            sequence: input.sequence,
            random: input.random,
            message_id: (0x01_00_00_00_u64 << 32) | u64::from(input.random),
            timestamp: input.timestamp,
            reserved: 0,
            client_sequence: input.client_sequence,
        }),
        settings: Some(PrivateRecallSettings {
            first: false,
            second: false,
        }),
        reserved: false,
    }
    .encode_to_vec())
}

#[derive(Clone, PartialEq, Message)]
struct GroupRecallRequest {
    #[prost(uint32, tag = "1")]
    kind: u32,
    #[prost(uint64, tag = "2")]
    group_uin: u64,
    #[prost(message, optional, tag = "3")]
    message: Option<GroupRecallMessage>,
    #[prost(message, optional, tag = "4")]
    settings: Option<GroupRecallSettings>,
}

#[derive(Clone, PartialEq, Message)]
struct GroupRecallMessage {
    #[prost(uint64, tag = "1")]
    sequence: u64,
    #[prost(uint32, tag = "2")]
    random: u32,
    #[prost(uint32, tag = "3")]
    reserved: u32,
}

#[derive(Clone, PartialEq, Message)]
struct GroupRecallSettings {
    #[prost(uint32, tag = "1")]
    reserved: u32,
}

#[derive(Clone, PartialEq, Message)]
struct PrivateRecallRequest {
    #[prost(uint32, tag = "1")]
    kind: u32,
    #[prost(string, tag = "3")]
    target_uid: String,
    #[prost(message, optional, tag = "4")]
    message: Option<PrivateRecallMessage>,
    #[prost(message, optional, tag = "5")]
    settings: Option<PrivateRecallSettings>,
    #[prost(bool, tag = "6")]
    reserved: bool,
}

#[derive(Clone, PartialEq, Message)]
struct PrivateRecallMessage {
    #[prost(uint64, tag = "1")]
    sequence: u64,
    #[prost(uint32, tag = "2")]
    random: u32,
    #[prost(uint64, tag = "3")]
    message_id: u64,
    #[prost(uint32, tag = "4")]
    timestamp: u32,
    #[prost(uint32, tag = "5")]
    reserved: u32,
    #[prost(uint64, tag = "6")]
    client_sequence: u64,
}

#[derive(Clone, PartialEq, Message)]
struct PrivateRecallSettings {
    #[prost(bool, tag = "1")]
    first: bool,
    #[prost(bool, tag = "2")]
    second: bool,
}
