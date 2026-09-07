use std::collections::BTreeMap;

use prost::Message;

use crate::{MessageClass, MessageDecodeError, MessageEnvelope, event_payload};

const GROUP_POKE_SUBTYPE: u32 = 20;
const FRIEND_POKE_SUBTYPE: u32 = 290;
const POKE_BUSINESS_TYPE: u64 = 12;
const MAX_TEMPLATE_ENTRIES: usize = 32;
const MAX_TEMPLATE_BYTES: usize = 2048;

/// Origin of one QQ poke notice.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PokeScope {
    /// A group poke.
    Group(u32),
    /// A direct-contact poke.
    Friend,
}

/// One authenticated QQ poke with numeric participants.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PokeNotice {
    scope: PokeScope,
    operator_id: u32,
    target_id: u32,
    action: String,
    suffix: String,
    image_url: String,
}

impl PokeNotice {
    /// Returns the conversation scope.
    #[must_use]
    pub const fn scope(&self) -> PokeScope {
        self.scope
    }
    /// Returns the numeric operator identifier.
    #[must_use]
    pub const fn operator_id(&self) -> u32 {
        self.operator_id
    }
    /// Returns the numeric target identifier.
    #[must_use]
    pub const fn target_id(&self) -> u32 {
        self.target_id
    }
    /// Returns QQ's action description.
    #[must_use]
    pub fn action(&self) -> &str {
        &self.action
    }
    /// Returns QQ's suffix description.
    #[must_use]
    pub fn suffix(&self) -> &str {
        &self.suffix
    }
    /// Returns QQ's optional action image URL.
    #[must_use]
    pub fn image_url(&self) -> &str {
        &self.image_url
    }
}

/// Decodes the frozen-52194 group and friend poke shapes.
///
/// # Errors
///
/// Returns an error when a matching poke is malformed, oversized, or lacks numeric identities.
pub fn decode_poke_notice(
    envelope: &MessageEnvelope,
) -> Result<Option<PokeNotice>, MessageDecodeError> {
    let (scope, general) = match (envelope.class(), envelope.sub_type()) {
        (MessageClass::GroupEvent, GROUP_POKE_SUBTYPE) => {
            let content = envelope.payload().content().ok_or(MessageDecodeError)?;
            let (group_id, proto) = event_payload::split(content).ok_or(MessageDecodeError)?;
            if group_id == 0 {
                return Err(MessageDecodeError);
            }
            let body = NotifyWire::decode(proto).map_err(|_error| MessageDecodeError)?;
            (
                PokeScope::Group(group_id),
                body.general.ok_or(MessageDecodeError)?,
            )
        }
        (MessageClass::FriendEvent, FRIEND_POKE_SUBTYPE) => {
            let content = envelope.payload().content().ok_or(MessageDecodeError)?;
            (
                PokeScope::Friend,
                GeneralWire::decode(content).map_err(|_error| MessageDecodeError)?,
            )
        }
        _ => return Ok(None),
    };
    if general.business_type != POKE_BUSINESS_TYPE
        || general.parameters.is_empty()
        || general.parameters.len() > MAX_TEMPLATE_ENTRIES
    {
        return Err(MessageDecodeError);
    }
    let parameters = template_map(general.parameters)?;
    let operator_id = numeric_parameter(&parameters, "uin_str1")?;
    let target_id = numeric_parameter(&parameters, "uin_str2")?;
    let action = parameters
        .get("action_str")
        .or_else(|| parameters.get("alt_str1"))
        .cloned()
        .unwrap_or_default();
    let suffix = parameters.get("suffix_str").cloned().unwrap_or_default();
    let image_url = parameters
        .get("action_img_url")
        .cloned()
        .unwrap_or_default();
    Ok(Some(PokeNotice {
        scope,
        operator_id,
        target_id,
        action,
        suffix,
        image_url,
    }))
}

fn template_map(
    parameters: Vec<TemplateWire>,
) -> Result<BTreeMap<String, String>, MessageDecodeError> {
    let mut result = BTreeMap::new();
    for parameter in parameters {
        if parameter.name.is_empty()
            || parameter.name.len() > 64
            || parameter.value.len() > MAX_TEMPLATE_BYTES
            || parameter.name.chars().any(char::is_control)
            || parameter.value.contains('\0')
            || result.insert(parameter.name, parameter.value).is_some()
        {
            return Err(MessageDecodeError);
        }
    }
    Ok(result)
}

fn numeric_parameter(
    parameters: &BTreeMap<String, String>,
    name: &str,
) -> Result<u32, MessageDecodeError> {
    parameters
        .get(name)
        .filter(|value| !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit()))
        .and_then(|value| value.parse::<u32>().ok())
        .filter(|value| *value != 0)
        .ok_or(MessageDecodeError)
}

#[derive(Clone, PartialEq, Message)]
struct NotifyWire {
    #[prost(message, optional, tag = "26")]
    general: Option<GeneralWire>,
}

#[derive(Clone, PartialEq, Message)]
struct GeneralWire {
    #[prost(uint64, tag = "1")]
    business_type: u64,
    #[prost(message, repeated, tag = "7")]
    parameters: Vec<TemplateWire>,
}

#[derive(Clone, PartialEq, Message)]
struct TemplateWire {
    #[prost(string, tag = "1")]
    name: String,
    #[prost(string, tag = "2")]
    value: String,
}
