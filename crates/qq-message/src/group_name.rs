use prost::Message;

use crate::{MessageClass, MessageDecodeError, MessageEnvelope, event_payload};

const GROUP_EVENT_SUBTYPE: u32 = 16;
const GROUP_NAME_KIND: u32 = 12;
const MAX_GROUP_NAME_BYTES: usize = 512;

/// One authenticated QQ group-name change.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroupNameChange {
    group_id: u32,
    name: String,
}

impl GroupNameChange {
    /// Returns the numeric QQ group identifier.
    #[must_use]
    pub const fn group_id(&self) -> u32 {
        self.group_id
    }

    /// Returns the new group name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }
}

/// Decodes the frozen-52194 group-name change shape.
///
/// # Errors
///
/// Returns an error when a matching notice has contradictory framing or unsafe text.
pub fn decode_group_name_change(
    envelope: &MessageEnvelope,
) -> Result<Option<GroupNameChange>, MessageDecodeError> {
    if envelope.class() != MessageClass::GroupEvent || envelope.sub_type() != GROUP_EVENT_SUBTYPE {
        return Ok(None);
    }
    let Some(content) = envelope.payload().content() else {
        return Ok(None);
    };
    let Some((prefixed_group, proto)) = event_payload::split(content) else {
        return Ok(None);
    };
    let Ok(body) = NoticeWire::decode(proto) else {
        return Ok(None);
    };
    if body.kind != Some(GROUP_NAME_KIND) {
        return Ok(None);
    }
    let change =
        NameWire::decode(body.event_param.as_slice()).map_err(|_error| MessageDecodeError)?;
    if prefixed_group == 0
        || body.group_id != prefixed_group
        || change.name.is_empty()
        || change.name.len() > MAX_GROUP_NAME_BYTES
        || change.name.contains('\0')
    {
        return Err(MessageDecodeError);
    }
    Ok(Some(GroupNameChange {
        group_id: body.group_id,
        name: change.name,
    }))
}

#[derive(Clone, PartialEq, Message)]
struct NoticeWire {
    #[prost(uint32, tag = "4")]
    group_id: u32,
    #[prost(bytes = "vec", tag = "5")]
    event_param: Vec<u8>,
    #[prost(uint32, optional, tag = "13")]
    kind: Option<u32>,
}

#[derive(Clone, PartialEq, Message)]
struct NameWire {
    #[prost(string, tag = "2")]
    name: String,
}
