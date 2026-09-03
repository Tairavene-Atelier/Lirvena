use serde_json::Value;

use crate::{MessageParseError, MessageSegment, parse_message};

const MAX_NODES: usize = 100;
const MAX_NAME_BYTES: usize = 512;
const MAX_FORWARD_BYTES: usize = 16 * 1024 * 1024;

/// One validated custom node supplied to a merged-forward action.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ForwardNode {
    user_id: u32,
    nickname: String,
    content: Vec<MessageSegment>,
}

impl ForwardNode {
    /// Returns the displayed numeric sender identity.
    #[must_use]
    pub const fn user_id(&self) -> u32 {
        self.user_id
    }

    /// Returns the displayed sender name.
    #[must_use]
    pub fn nickname(&self) -> &str {
        &self.nickname
    }

    /// Returns the parsed message segments in wire order.
    #[must_use]
    pub fn content(&self) -> &[MessageSegment] {
        &self.content
    }
}

/// Parses the custom-node form accepted by `OneBot` merged-forward actions.
///
/// Both `user_id`/`nickname` and the established `uin`/`name` aliases are accepted. Message
/// content may be a CQ string, one segment object, or a segment array.
///
/// # Errors
///
/// Returns an error for reference-only nodes, malformed aliases, unsafe names, invalid message
/// syntax, or any count/size limit violation.
pub fn parse_forward_nodes(value: &Value) -> Result<Vec<ForwardNode>, MessageParseError> {
    if serde_json::to_vec(value)
        .map_err(|_error| MessageParseError)?
        .len()
        > MAX_FORWARD_BYTES
    {
        return Err(MessageParseError);
    }
    let nodes = value.as_array().ok_or(MessageParseError)?;
    if nodes.is_empty() || nodes.len() > MAX_NODES {
        return Err(MessageParseError);
    }
    nodes.iter().map(parse_node).collect()
}

fn parse_node(value: &Value) -> Result<ForwardNode, MessageParseError> {
    let outer = value.as_object().ok_or(MessageParseError)?;
    if outer.get("type").and_then(Value::as_str) != Some("node") {
        return Err(MessageParseError);
    }
    let data = outer
        .get("data")
        .and_then(Value::as_object)
        .ok_or(MessageParseError)?;
    let user_id = alias(data.get("user_id"), data.get("uin"), parse_u32)?;
    let nickname = alias(data.get("nickname"), data.get("name"), parse_name)?;
    let content = data.get("content").ok_or(MessageParseError)?;
    let normalized;
    let content = if content.is_object() {
        normalized = Value::Array(vec![content.clone()]);
        &normalized
    } else {
        content
    };
    Ok(ForwardNode {
        user_id,
        nickname,
        content: parse_message(content, false)?,
    })
}

fn alias<T: Eq>(
    primary: Option<&Value>,
    compatibility: Option<&Value>,
    parse: impl Fn(&Value) -> Option<T>,
) -> Result<T, MessageParseError> {
    match (primary.and_then(&parse), compatibility.and_then(parse)) {
        (Some(primary), Some(compatibility)) if primary != compatibility => Err(MessageParseError),
        (Some(value), _) | (_, Some(value)) => Ok(value),
        (None, None) => Err(MessageParseError),
    }
}

fn parse_u32(value: &Value) -> Option<u32> {
    let number = match value {
        Value::Number(number) => number.as_u64(),
        Value::String(number) => number.parse().ok(),
        _ => None,
    }?;
    u32::try_from(number).ok().filter(|value| *value != 0)
}

fn parse_name(value: &Value) -> Option<String> {
    let value = value.as_str()?;
    (!value.trim().is_empty()
        && value.len() <= MAX_NAME_BYTES
        && !value.chars().any(char::is_control))
    .then(|| value.to_owned())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::parse_forward_nodes;

    #[test]
    fn aliases_and_all_content_shapes_are_bounded() -> Result<(), Box<dyn std::error::Error>> {
        let nodes = parse_forward_nodes(&json!([
            {"type":"node","data":{"user_id":"42","nickname":"Alice","content":"hello"}},
            {"type":"node","data":{"uin":43,"name":"Bob","content":{"type":"face","data":{"id":14}}}},
            {"type":"node","data":{"user_id":44,"nickname":"Carol","content":[{"type":"text","data":{"text":"x"}}]}}
        ]))?;
        assert_eq!(nodes.len(), 3);
        assert_eq!(nodes[0].user_id(), 42);
        assert_eq!(nodes[1].content()[0].kind(), "face");
        Ok(())
    }

    #[test]
    fn reference_nodes_and_conflicting_aliases_fail_closed() {
        assert!(parse_forward_nodes(&json!([{"type":"node","data":{"id":7}}])).is_err());
        assert!(
            parse_forward_nodes(&json!([{
                "type":"node",
                "data":{"user_id":42,"uin":43,"nickname":"Alice","content":"x"}
            }]))
            .is_err()
        );
    }
}
