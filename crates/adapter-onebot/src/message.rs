use serde_json::{Map, Value};

const MAX_SEGMENTS: usize = 256;
const MAX_MESSAGE_BYTES: usize = 1024 * 1024;
const MAX_KIND_BYTES: usize = 128;

/// One syntactically valid `OneBot` message segment without a business whitelist.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MessageSegment {
    kind: String,
    data: Map<String, Value>,
}

impl MessageSegment {
    /// Segment type exactly as supplied by the `OneBot` caller.
    #[must_use]
    pub fn kind(&self) -> &str {
        &self.kind
    }

    /// Segment data exactly as supplied or decoded from CQ syntax.
    #[must_use]
    pub const fn data(&self) -> &Map<String, Value> {
        &self.data
    }
}

/// Opaque `OneBot` message syntax failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MessageParseError;

impl core::fmt::Display for MessageParseError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("OneBot message syntax is invalid")
    }
}

impl std::error::Error for MessageParseError {}

/// Parses `OneBot` array messages and CQ strings while preserving arbitrary segment types.
///
/// # Errors
///
/// Returns an error for invalid shapes, malformed CQ fields, control-character types, or bounded
/// size/count violations.
pub fn parse_message(
    value: &Value,
    auto_escape: bool,
) -> Result<Vec<MessageSegment>, MessageParseError> {
    if encoded_size(value)? > MAX_MESSAGE_BYTES {
        return Err(MessageParseError);
    }
    let segments = match value {
        Value::String(text) if auto_escape => vec![text_segment(text.clone())],
        Value::String(text) => parse_cq(text)?,
        Value::Array(segments) => segments
            .iter()
            .map(parse_json_segment)
            .collect::<Result<Vec<_>, _>>()?,
        _ => return Err(MessageParseError),
    };
    if segments.is_empty() || segments.len() > MAX_SEGMENTS {
        return Err(MessageParseError);
    }
    Ok(segments)
}

fn parse_json_segment(value: &Value) -> Result<MessageSegment, MessageParseError> {
    let object = value.as_object().ok_or(MessageParseError)?;
    let kind = object
        .get("type")
        .and_then(Value::as_str)
        .ok_or(MessageParseError)?;
    validate_kind(kind)?;
    let data = object
        .get("data")
        .and_then(Value::as_object)
        .cloned()
        .ok_or(MessageParseError)?;
    Ok(MessageSegment {
        kind: kind.to_owned(),
        data,
    })
}

fn parse_cq(input: &str) -> Result<Vec<MessageSegment>, MessageParseError> {
    let mut segments = Vec::new();
    let mut remaining = input;
    while let Some(start) = remaining.find("[CQ:") {
        if start != 0 {
            segments.push(text_segment(unescape(&remaining[..start], false)));
        }
        let code = &remaining[start + 4..];
        let end = code.find(']').ok_or(MessageParseError)?;
        segments.push(parse_cq_code(&code[..end])?);
        remaining = &code[end + 1..];
        if segments.len() > MAX_SEGMENTS {
            return Err(MessageParseError);
        }
    }
    if !remaining.is_empty() {
        segments.push(text_segment(unescape(remaining, false)));
    }
    if segments.iter().any(|segment| {
        segment.kind == "text" && segment.data.get("text").and_then(Value::as_str) == Some("")
    }) {
        segments.retain(|segment| {
            segment.kind != "text" || segment.data.get("text").and_then(Value::as_str) != Some("")
        });
    }
    Ok(segments)
}

fn parse_cq_code(input: &str) -> Result<MessageSegment, MessageParseError> {
    let mut fields = input.split(',');
    let kind = fields.next().ok_or(MessageParseError)?;
    validate_kind(kind)?;
    let mut data = Map::new();
    for field in fields {
        let (key, value) = field.split_once('=').ok_or(MessageParseError)?;
        if key.is_empty()
            || key.len() > MAX_KIND_BYTES
            || key
                .chars()
                .any(|character| character.is_control() || character == ',')
            || data
                .insert(key.to_owned(), Value::String(unescape(value, true)))
                .is_some()
        {
            return Err(MessageParseError);
        }
    }
    Ok(MessageSegment {
        kind: kind.to_owned(),
        data,
    })
}

fn text_segment(text: String) -> MessageSegment {
    MessageSegment {
        kind: "text".to_owned(),
        data: Map::from_iter([("text".to_owned(), Value::String(text))]),
    }
}

fn validate_kind(kind: &str) -> Result<(), MessageParseError> {
    if kind.is_empty()
        || kind.len() > MAX_KIND_BYTES
        || kind
            .chars()
            .any(|character| character.is_control() || matches!(character, ',' | '[' | ']'))
    {
        Err(MessageParseError)
    } else {
        Ok(())
    }
}

fn unescape(value: &str, parameter: bool) -> String {
    let value = if parameter {
        value.replace("&#44;", ",")
    } else {
        value.to_owned()
    };
    value
        .replace("&#91;", "[")
        .replace("&#93;", "]")
        .replace("&amp;", "&")
}

fn encoded_size(value: &Value) -> Result<usize, MessageParseError> {
    serde_json::to_vec(value)
        .map(|value| value.len())
        .map_err(|_error| MessageParseError)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn json_preserves_arbitrary_extension_segments() -> Result<(), MessageParseError> {
        let segments = parse_message(
            &json!([{"type": "vendor.extension", "data": {"value": 7}}]),
            false,
        )?;
        assert_eq!(segments[0].kind(), "vendor.extension");
        assert_eq!(segments[0].data().get("value"), Some(&json!(7)));
        Ok(())
    }

    #[test]
    fn cq_decodes_text_parameters_and_multiple_segments() -> Result<(), MessageParseError> {
        let segments = parse_message(
            &json!("a&amp;b[CQ:at,qq=123,name=x&#44;y][CQ:face,id=14]"),
            false,
        )?;
        assert_eq!(segments.len(), 3);
        assert_eq!(segments[0].data().get("text"), Some(&json!("a&b")));
        assert_eq!(segments[1].data().get("name"), Some(&json!("x,y")));
        assert_eq!(segments[2].kind(), "face");
        Ok(())
    }

    #[test]
    fn auto_escape_keeps_cq_source_as_text() -> Result<(), MessageParseError> {
        let segments = parse_message(&json!("[CQ:face,id=14]"), true)?;
        assert_eq!(segments, vec![text_segment("[CQ:face,id=14]".to_owned())]);
        Ok(())
    }
}
