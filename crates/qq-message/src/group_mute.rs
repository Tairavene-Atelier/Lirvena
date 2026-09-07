use prost::Message;

use crate::{MessageClass, MessageDecodeError, MessageEnvelope};

const GROUP_MUTE_SUBTYPE: u32 = 12;
const MAX_UID_BYTES: usize = 128;

/// Authenticated Linux NT group mute change.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroupMute {
    group_id: u32,
    operator_uid: Option<String>,
    target_uid: Option<String>,
    duration: u32,
}

impl GroupMute {
    /// Returns the numeric QQ group identifier.
    #[must_use]
    pub const fn group_id(&self) -> u32 {
        self.group_id
    }

    /// Returns the current Linux NT UID of the operator when supplied.
    #[must_use]
    pub fn operator_uid(&self) -> Option<&str> {
        self.operator_uid.as_deref()
    }

    /// Returns the muted member UID, or `None` for a whole-group change.
    #[must_use]
    pub fn target_uid(&self) -> Option<&str> {
        self.target_uid.as_deref()
    }

    /// Returns the QQ mute duration in seconds; zero denotes unmute.
    #[must_use]
    pub const fn duration(&self) -> u32 {
        self.duration
    }
}

/// Decodes the frozen 52194 group-mute notice shape.
///
/// A missing target denotes the whole group. A zero duration denotes unmute.
///
/// # Errors
///
/// Returns an error when the authenticated notice is structurally invalid.
pub fn decode_group_mute(
    envelope: &MessageEnvelope,
) -> Result<Option<GroupMute>, MessageDecodeError> {
    if envelope.class() != MessageClass::GroupEvent || envelope.sub_type() != GROUP_MUTE_SUBTYPE {
        return Ok(None);
    }
    let content = envelope.payload().content().ok_or(MessageDecodeError)?;
    let wire = GroupMuteWire::decode(content).map_err(|_error| MessageDecodeError)?;
    let state = wire
        .data
        .and_then(|data| data.state)
        .ok_or(MessageDecodeError)?;
    if wire.group_id == 0 {
        return Err(MessageDecodeError);
    }
    let operator_uid = validate_optional_uid(wire.operator_uid)?;
    let target_uid = validate_optional_uid(state.target_uid)?;
    Ok(Some(GroupMute {
        group_id: wire.group_id,
        operator_uid,
        target_uid,
        duration: state.duration,
    }))
}

fn validate_optional_uid(value: Option<String>) -> Result<Option<String>, MessageDecodeError> {
    value
        .filter(|value| !value.is_empty())
        .map(|value| {
            if value.len() > MAX_UID_BYTES || value.chars().any(char::is_control) {
                Err(MessageDecodeError)
            } else {
                Ok(value)
            }
        })
        .transpose()
}

#[derive(Clone, PartialEq, Message)]
struct GroupMuteWire {
    #[prost(uint32, tag = "1")]
    group_id: u32,
    #[prost(string, optional, tag = "4")]
    operator_uid: Option<String>,
    #[prost(message, optional, tag = "5")]
    data: Option<GroupMuteDataWire>,
}

#[derive(Clone, PartialEq, Message)]
struct GroupMuteDataWire {
    #[prost(message, optional, tag = "3")]
    state: Option<GroupMuteStateWire>,
}

#[derive(Clone, PartialEq, Message)]
struct GroupMuteStateWire {
    #[prost(string, optional, tag = "1")]
    target_uid: Option<String>,
    #[prost(uint32, tag = "2")]
    duration: u32,
}
