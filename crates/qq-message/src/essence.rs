use prost::Message;

use crate::{MessageClass, MessageDecodeError, MessageEnvelope, event_payload};

const GROUP_GREY_TIP_SUBTYPE: u32 = 21;
const ESSENCE_TYPE: u32 = 27;

/// One authenticated group essence-message change.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroupEssence {
    group_id: u32,
    sequence: u32,
    random: u32,
    added: bool,
    sender_id: u32,
    operator_id: u32,
}

impl GroupEssence {
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
    /// Returns the target QQ message random value.
    #[must_use]
    pub const fn random(&self) -> u32 {
        self.random
    }
    /// Returns whether the essence marker was added.
    #[must_use]
    pub const fn is_added(&self) -> bool {
        self.added
    }
    /// Returns the original message sender.
    #[must_use]
    pub const fn sender_id(&self) -> u32 {
        self.sender_id
    }
    /// Returns the operator who changed the marker.
    #[must_use]
    pub const fn operator_id(&self) -> u32 {
        self.operator_id
    }
}

/// Decodes the frozen-52194 essence change shape.
///
/// # Errors
///
/// Returns an error for contradictory framing, missing correlation fields, or unknown operations.
pub fn decode_group_essence(
    envelope: &MessageEnvelope,
) -> Result<Option<GroupEssence>, MessageDecodeError> {
    if envelope.class() != MessageClass::GroupEvent || envelope.sub_type() != GROUP_GREY_TIP_SUBTYPE
    {
        return Ok(None);
    }
    let content = envelope.payload().content().ok_or(MessageDecodeError)?;
    let (prefixed_group, proto) = event_payload::split(content).ok_or(MessageDecodeError)?;
    let body = NoticeWire::decode(proto).map_err(|_error| MessageDecodeError)?;
    if body.kind != ESSENCE_TYPE {
        return Ok(None);
    }
    let essence = body.essence.ok_or(MessageDecodeError)?;
    if prefixed_group == 0
        || essence.group_id != prefixed_group
        || essence.sequence == 0
        || essence.random == 0
        || essence.sender_id == 0
        || essence.operator_id == 0
    {
        return Err(MessageDecodeError);
    }
    let added = match essence.operation {
        1 => true,
        2 => false,
        _ => return Err(MessageDecodeError),
    };
    Ok(Some(GroupEssence {
        group_id: essence.group_id,
        sequence: essence.sequence,
        random: essence.random,
        added,
        sender_id: essence.sender_id,
        operator_id: essence.operator_id,
    }))
}

#[derive(Clone, PartialEq, Message)]
struct NoticeWire {
    #[prost(uint32, tag = "1")]
    kind: u32,
    #[prost(message, optional, tag = "33")]
    essence: Option<EssenceWire>,
}

#[derive(Clone, Copy, PartialEq, Message)]
struct EssenceWire {
    #[prost(uint32, tag = "1")]
    group_id: u32,
    #[prost(uint32, tag = "2")]
    sequence: u32,
    #[prost(uint32, tag = "3")]
    random: u32,
    #[prost(uint32, tag = "4")]
    operation: u32,
    #[prost(uint32, tag = "5")]
    sender_id: u32,
    #[prost(uint32, tag = "6")]
    operator_id: u32,
}
