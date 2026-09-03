use prost::Message;

use crate::{MessageClass, MessageDecodeError, MessageEnvelope};

const GROUP_EVENT_SUBTYPE: u32 = 16;
const REACTION_FIELD_KIND: u32 = 35;
const MAX_UID_BYTES: usize = 128;
const MAX_CODE_BYTES: usize = 10;

/// One authenticated group-message reaction change.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroupReaction {
    group_id: u32,
    sequence: u32,
    operator_uid: String,
    add: bool,
    code: String,
    count: u32,
}

impl GroupReaction {
    /// Returns the numeric QQ group identifier.
    #[must_use]
    pub const fn group_id(&self) -> u32 {
        self.group_id
    }

    /// Returns the target QQ message sequence.
    #[must_use]
    pub const fn sequence(&self) -> u32 {
        self.sequence
    }

    /// Returns the current Linux NT UID of the operator.
    #[must_use]
    pub fn operator_uid(&self) -> &str {
        &self.operator_uid
    }

    /// Returns whether the reaction was added rather than removed.
    #[must_use]
    pub const fn is_add(&self) -> bool {
        self.add
    }

    /// Returns the decimal reaction identifier.
    #[must_use]
    pub fn code(&self) -> &str {
        &self.code
    }

    /// Returns QQ's aggregate count after the change.
    #[must_use]
    pub const fn count(&self) -> u32 {
        self.count
    }
}

/// Decodes the compiled Linux NT group-reaction notice shape.
///
/// # Errors
///
/// Returns an error when an envelope identified as a reaction contains contradictory or unsafe
/// fields. Other group-event subtypes and subtype-16 notices remain available to future decoders.
pub fn decode_group_reaction(
    envelope: &MessageEnvelope,
) -> Result<Option<GroupReaction>, MessageDecodeError> {
    if envelope.class() != MessageClass::GroupEvent || envelope.sub_type() != GROUP_EVENT_SUBTYPE {
        return Ok(None);
    }
    let Some(content) = envelope.payload().content() else {
        return Ok(None);
    };
    let Some((prefixed_group, proto)) = split_event_payload(content) else {
        return Ok(None);
    };
    let Ok(body) = ReactionNoticeWire::decode(proto) else {
        return Ok(None);
    };
    if body.kind != Some(REACTION_FIELD_KIND) {
        return Ok(None);
    }
    let reaction = body
        .reaction
        .and_then(|value| value.data)
        .and_then(|value| value.data)
        .ok_or(MessageDecodeError)?;
    let target = reaction.target.ok_or(MessageDecodeError)?;
    let data = reaction.data.ok_or(MessageDecodeError)?;
    if prefixed_group == 0
        || body.group_id != prefixed_group
        || target.sequence == 0
        || data.operator_uid.is_empty()
        || data.operator_uid.len() > MAX_UID_BYTES
        || data.operator_uid.chars().any(char::is_control)
        || data.code.is_empty()
        || data.code.len() > MAX_CODE_BYTES
        || data.code.bytes().any(|byte| !byte.is_ascii_digit())
        || data.code.parse::<u32>().ok().is_none_or(|value| value == 0)
    {
        return Err(MessageDecodeError);
    }
    let add = match data.kind {
        1 => true,
        2 => false,
        _ => return Err(MessageDecodeError),
    };
    Ok(Some(GroupReaction {
        group_id: body.group_id,
        sequence: target.sequence,
        operator_uid: data.operator_uid,
        add,
        code: data.code,
        count: data.count,
    }))
}

fn split_event_payload(input: &[u8]) -> Option<(u32, &[u8])> {
    let group = u32::from_be_bytes(input.get(..4)?.try_into().ok()?);
    let length = usize::from(u16::from_be_bytes(input.get(5..7)?.try_into().ok()?));
    let end = 7_usize.checked_add(length)?;
    (end == input.len()).then(|| (group, &input[7..end]))
}

#[derive(Clone, PartialEq, Message)]
struct ReactionNoticeWire {
    #[prost(uint32, tag = "4")]
    group_id: u32,
    #[prost(uint32, optional, tag = "13")]
    kind: Option<u32>,
    #[prost(message, optional, tag = "44")]
    reaction: Option<ReactionLevelZeroWire>,
}

#[derive(Clone, PartialEq, Message)]
struct ReactionLevelZeroWire {
    #[prost(message, optional, tag = "1")]
    data: Option<ReactionLevelOneWire>,
}

#[derive(Clone, PartialEq, Message)]
struct ReactionLevelOneWire {
    #[prost(message, optional, tag = "1")]
    data: Option<ReactionBodyWire>,
}

#[derive(Clone, PartialEq, Message)]
struct ReactionBodyWire {
    #[prost(message, optional, tag = "2")]
    target: Option<ReactionTargetWire>,
    #[prost(message, optional, tag = "3")]
    data: Option<ReactionDataWire>,
}

#[derive(Clone, PartialEq, Message)]
struct ReactionTargetWire {
    #[prost(uint32, tag = "1")]
    sequence: u32,
}

#[derive(Clone, PartialEq, Message)]
struct ReactionDataWire {
    #[prost(string, tag = "1")]
    code: String,
    #[prost(uint32, tag = "3")]
    count: u32,
    #[prost(string, tag = "4")]
    operator_uid: String,
    #[prost(uint32, tag = "5")]
    kind: u32,
}
