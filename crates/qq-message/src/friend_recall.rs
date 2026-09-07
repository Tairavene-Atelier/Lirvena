use prost::Message;

use crate::{MessageClass, MessageDecodeError, MessageEnvelope};

const FRIEND_RECALL_SUBTYPE: u32 = 138;
const MAX_UID_BYTES: usize = 128;
const MAX_TIP_BYTES: usize = 2048;

/// One authenticated direct-message recall correlation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FriendRecall {
    from_uid: String,
    client_sequence: u32,
    random: u32,
    timestamp: u32,
    tip: String,
}

impl FriendRecall {
    /// Returns the current Linux NT UID of the recalling author.
    #[must_use]
    pub fn from_uid(&self) -> &str {
        &self.from_uid
    }
    /// Returns the original client sequence.
    #[must_use]
    pub const fn client_sequence(&self) -> u32 {
        self.client_sequence
    }
    /// Returns the original random value.
    #[must_use]
    pub const fn random(&self) -> u32 {
        self.random
    }
    /// Returns the original Unix timestamp.
    #[must_use]
    pub const fn timestamp(&self) -> u32 {
        self.timestamp
    }
    /// Returns QQ's bounded recall tip.
    #[must_use]
    pub fn tip(&self) -> &str {
        &self.tip
    }
}

/// Decodes the frozen-52194 friend-recall notice shape.
///
/// # Errors
///
/// Returns an error for malformed, incomplete, or unsafe fields.
pub fn decode_friend_recall(
    envelope: &MessageEnvelope,
) -> Result<Option<FriendRecall>, MessageDecodeError> {
    if envelope.class() != MessageClass::FriendEvent || envelope.sub_type() != FRIEND_RECALL_SUBTYPE
    {
        return Ok(None);
    }
    let content = envelope.payload().content().ok_or(MessageDecodeError)?;
    let info = FriendRecallWire::decode(content)
        .map_err(|_error| MessageDecodeError)?
        .info
        .ok_or(MessageDecodeError)?;
    let tip = info.tip.and_then(|value| value.tip).unwrap_or_default();
    if info.from_uid.is_empty()
        || info.from_uid.len() > MAX_UID_BYTES
        || info.from_uid.chars().any(char::is_control)
        || info.client_sequence == 0
        || info.random == 0
        || info.timestamp == 0
        || tip.len() > MAX_TIP_BYTES
        || tip.chars().any(char::is_control)
    {
        return Err(MessageDecodeError);
    }
    Ok(Some(FriendRecall {
        from_uid: info.from_uid,
        client_sequence: info.client_sequence,
        random: info.random,
        timestamp: info.timestamp,
        tip,
    }))
}

#[derive(Clone, PartialEq, Message)]
struct FriendRecallWire {
    #[prost(message, optional, tag = "1")]
    info: Option<FriendRecallInfoWire>,
}

#[derive(Clone, PartialEq, Message)]
struct FriendRecallInfoWire {
    #[prost(string, tag = "1")]
    from_uid: String,
    #[prost(uint32, tag = "3")]
    client_sequence: u32,
    #[prost(uint32, tag = "5")]
    timestamp: u32,
    #[prost(uint32, tag = "6")]
    random: u32,
    #[prost(message, optional, tag = "13")]
    tip: Option<FriendRecallTipWire>,
}

#[derive(Clone, PartialEq, Message)]
struct FriendRecallTipWire {
    #[prost(string, optional, tag = "2")]
    tip: Option<String>,
}
