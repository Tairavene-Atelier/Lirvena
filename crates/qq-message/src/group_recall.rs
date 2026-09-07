use prost::Message;

use crate::{MessageClass, MessageDecodeError, MessageEnvelope};

const GROUP_RECALL_SUBTYPE: u32 = 17;
const MAX_RECALLS: usize = 32;
const MAX_UID_BYTES: usize = 128;
const MAX_TIP_BYTES: usize = 2048;

/// One recalled QQ group message carried by an authenticated notice.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroupRecall {
    group_id: u32,
    sequence: u32,
    random: u32,
    author_uid: String,
    operator_uid: Option<String>,
    tip: String,
}

impl GroupRecall {
    /// Returns the numeric QQ group identifier.
    #[must_use]
    pub const fn group_id(&self) -> u32 {
        self.group_id
    }
    /// Returns the recalled QQ message sequence.
    #[must_use]
    pub const fn sequence(&self) -> u32 {
        self.sequence
    }
    /// Returns the recalled QQ message random value.
    #[must_use]
    pub const fn random(&self) -> u32 {
        self.random
    }
    /// Returns the current Linux NT UID of the message author.
    #[must_use]
    pub fn author_uid(&self) -> &str {
        &self.author_uid
    }
    /// Returns the current Linux NT UID of the operator when supplied.
    #[must_use]
    pub fn operator_uid(&self) -> Option<&str> {
        self.operator_uid.as_deref()
    }
    /// Returns QQ's bounded human-readable recall tip.
    #[must_use]
    pub fn tip(&self) -> &str {
        &self.tip
    }
}

/// Decodes all frozen-52194 group recalls in one authenticated notice.
///
/// # Errors
///
/// Returns an error for malformed framing, excessive entries, missing correlations, or unsafe text.
pub fn decode_group_recalls(
    envelope: &MessageEnvelope,
) -> Result<Option<Vec<GroupRecall>>, MessageDecodeError> {
    if envelope.class() != MessageClass::GroupEvent || envelope.sub_type() != GROUP_RECALL_SUBTYPE {
        return Ok(None);
    }
    let content = envelope.payload().content().ok_or(MessageDecodeError)?;
    let (prefixed_group, proto) = split_event_payload(content).ok_or(MessageDecodeError)?;
    let body = NoticeWire::decode(proto).map_err(|_error| MessageDecodeError)?;
    if prefixed_group == 0 || body.group_id != prefixed_group {
        return Err(MessageDecodeError);
    }
    let recall = body.recall.ok_or(MessageDecodeError)?;
    if recall.messages.is_empty() || recall.messages.len() > MAX_RECALLS {
        return Err(MessageDecodeError);
    }
    let operator_uid = validate_optional_uid(recall.operator_uid)?;
    let tip = recall.tip.and_then(|value| value.tip).unwrap_or_default();
    validate_text(&tip, MAX_TIP_BYTES)?;
    recall
        .messages
        .into_iter()
        .map(|message| {
            validate_uid(&message.author_uid)?;
            if message.sequence == 0 || message.random == 0 {
                return Err(MessageDecodeError);
            }
            Ok(GroupRecall {
                group_id: body.group_id,
                sequence: message.sequence,
                random: message.random,
                author_uid: message.author_uid,
                operator_uid: operator_uid.clone(),
                tip: tip.clone(),
            })
        })
        .collect::<Result<Vec<_>, _>>()
        .map(Some)
}

fn split_event_payload(input: &[u8]) -> Option<(u32, &[u8])> {
    let group = u32::from_be_bytes(input.get(..4)?.try_into().ok()?);
    let length = usize::from(u16::from_be_bytes(input.get(5..7)?.try_into().ok()?));
    let end = 7_usize.checked_add(length)?;
    (end == input.len()).then(|| (group, &input[7..end]))
}

fn validate_optional_uid(value: Option<String>) -> Result<Option<String>, MessageDecodeError> {
    value
        .filter(|value| !value.is_empty())
        .map(|value| {
            validate_uid(&value)?;
            Ok(value)
        })
        .transpose()
}

fn validate_uid(value: &str) -> Result<(), MessageDecodeError> {
    validate_text(value, MAX_UID_BYTES)?;
    if value.is_empty() {
        Err(MessageDecodeError)
    } else {
        Ok(())
    }
}

fn validate_text(value: &str, maximum: usize) -> Result<(), MessageDecodeError> {
    if value.len() > maximum || value.chars().any(char::is_control) {
        Err(MessageDecodeError)
    } else {
        Ok(())
    }
}

#[derive(Clone, PartialEq, Message)]
struct NoticeWire {
    #[prost(uint32, tag = "4")]
    group_id: u32,
    #[prost(message, optional, tag = "11")]
    recall: Option<RecallWire>,
}

#[derive(Clone, PartialEq, Message)]
struct RecallWire {
    #[prost(string, optional, tag = "1")]
    operator_uid: Option<String>,
    #[prost(message, repeated, tag = "3")]
    messages: Vec<RecallMessageWire>,
    #[prost(message, optional, tag = "9")]
    tip: Option<RecallTipWire>,
}

#[derive(Clone, PartialEq, Message)]
struct RecallMessageWire {
    #[prost(uint32, tag = "1")]
    sequence: u32,
    #[prost(uint32, tag = "3")]
    random: u32,
    #[prost(string, tag = "6")]
    author_uid: String,
}

#[derive(Clone, PartialEq, Message)]
struct RecallTipWire {
    #[prost(string, optional, tag = "2")]
    tip: Option<String>,
}
