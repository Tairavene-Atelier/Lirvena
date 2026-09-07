use prost::Message;
use serde::Deserialize;

use crate::MessageDecodeError;

const MAX_MARKDOWN_BYTES: usize = 16 * 1024;
const MAX_KEYBOARD_BYTES: usize = 32 * 1024;
const MAX_ROWS: usize = 5;
const MAX_BUTTONS_PER_ROW: usize = 5;
const MAX_PERMISSION_IDS: usize = 64;

pub(super) fn encode_markdown(content: &str) -> Result<Vec<u8>, MessageDecodeError> {
    if content.is_empty() || content.len() > MAX_MARKDOWN_BYTES || content.contains('\0') {
        return Err(MessageDecodeError);
    }
    Ok(MarkdownBody {
        content: content.to_owned(),
    }
    .encode_to_vec())
}

pub(super) fn encode_keyboard(input: &str) -> Result<Vec<u8>, MessageDecodeError> {
    if input.is_empty() || input.len() > MAX_KEYBOARD_BYTES {
        return Err(MessageDecodeError);
    }
    let keyboard: Keyboard = serde_json::from_str(input).map_err(|_error| MessageDecodeError)?;
    validate_keyboard(&keyboard)?;
    Ok(KeyboardEnvelope {
        keyboard: Some(keyboard.into()),
    }
    .encode_to_vec())
}

fn validate_keyboard(keyboard: &Keyboard) -> Result<(), MessageDecodeError> {
    if keyboard.rows.is_empty() || keyboard.rows.len() > MAX_ROWS {
        return Err(MessageDecodeError);
    }
    for row in &keyboard.rows {
        if row.buttons.is_empty() || row.buttons.len() > MAX_BUTTONS_PER_ROW {
            return Err(MessageDecodeError);
        }
        for button in &row.buttons {
            validate_text(&button.id, 128, false)?;
            validate_text(&button.render_data.label, 256, false)?;
            validate_text(&button.render_data.visited_label, 256, true)?;
            validate_text(&button.action.unsupport_tips, 512, true)?;
            validate_text(&button.action.data, 2_048, true)?;
            if button.action.permission.specify_role_ids.len() > MAX_PERMISSION_IDS
                || button.action.permission.specify_user_ids.len() > MAX_PERMISSION_IDS
            {
                return Err(MessageDecodeError);
            }
            for value in button
                .action
                .permission
                .specify_role_ids
                .iter()
                .chain(&button.action.permission.specify_user_ids)
            {
                validate_text(value, 128, false)?;
            }
        }
    }
    Ok(())
}

fn validate_text(
    value: &str,
    max_bytes: usize,
    allow_empty: bool,
) -> Result<(), MessageDecodeError> {
    if (!allow_empty && value.is_empty())
        || value.len() > max_bytes
        || value.chars().any(char::is_control)
    {
        return Err(MessageDecodeError);
    }
    Ok(())
}

#[derive(Clone, PartialEq, Message)]
struct MarkdownBody {
    #[prost(string, tag = "1")]
    content: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Keyboard {
    rows: Vec<Row>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Row {
    buttons: Vec<Button>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Button {
    id: String,
    render_data: RenderData,
    action: ButtonAction,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RenderData {
    label: String,
    #[serde(default)]
    visited_label: String,
    style: i32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ButtonAction {
    #[serde(rename = "type")]
    kind: i32,
    permission: Permission,
    #[serde(default)]
    unsupport_tips: String,
    #[serde(default)]
    data: String,
    reply: Option<bool>,
    enter: Option<bool>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Permission {
    #[serde(rename = "type")]
    kind: i32,
    #[serde(default)]
    specify_role_ids: Vec<String>,
    #[serde(default)]
    specify_user_ids: Vec<String>,
}

#[derive(Clone, PartialEq, Message)]
struct KeyboardEnvelope {
    #[prost(message, optional, tag = "1")]
    keyboard: Option<KeyboardWire>,
}

#[derive(Clone, PartialEq, Message)]
struct KeyboardWire {
    #[prost(message, repeated, tag = "1")]
    rows: Vec<RowWire>,
}

#[derive(Clone, PartialEq, Message)]
struct RowWire {
    #[prost(message, repeated, tag = "1")]
    buttons: Vec<ButtonWire>,
}

#[derive(Clone, PartialEq, Message)]
struct ButtonWire {
    #[prost(string, tag = "1")]
    id: String,
    #[prost(message, optional, tag = "2")]
    render: Option<RenderWire>,
    #[prost(message, optional, tag = "3")]
    action: Option<ActionWire>,
}

#[derive(Clone, PartialEq, Message)]
struct RenderWire {
    #[prost(string, tag = "1")]
    label: String,
    #[prost(string, tag = "2")]
    visited_label: String,
    #[prost(int32, tag = "3")]
    style: i32,
}

#[derive(Clone, PartialEq, Message)]
struct ActionWire {
    #[prost(int32, tag = "1")]
    kind: i32,
    #[prost(message, optional, tag = "2")]
    permission: Option<PermissionWire>,
    #[prost(string, tag = "4")]
    unsupported_tips: String,
    #[prost(string, tag = "5")]
    data: String,
    #[prost(bool, optional, tag = "7")]
    reply: Option<bool>,
    #[prost(bool, optional, tag = "8")]
    enter: Option<bool>,
}

#[derive(Clone, PartialEq, Message)]
struct PermissionWire {
    #[prost(int32, tag = "1")]
    kind: i32,
    #[prost(string, repeated, tag = "2")]
    role_ids: Vec<String>,
    #[prost(string, repeated, tag = "3")]
    user_ids: Vec<String>,
}

impl From<Keyboard> for KeyboardWire {
    fn from(value: Keyboard) -> Self {
        Self {
            rows: value.rows.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<Row> for RowWire {
    fn from(value: Row) -> Self {
        Self {
            buttons: value.buttons.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<Button> for ButtonWire {
    fn from(value: Button) -> Self {
        Self {
            id: value.id,
            render: Some(value.render_data.into()),
            action: Some(value.action.into()),
        }
    }
}

impl From<RenderData> for RenderWire {
    fn from(value: RenderData) -> Self {
        Self {
            label: value.label,
            visited_label: value.visited_label,
            style: value.style,
        }
    }
}

impl From<ButtonAction> for ActionWire {
    fn from(value: ButtonAction) -> Self {
        Self {
            kind: value.kind,
            permission: Some(value.permission.into()),
            unsupported_tips: value.unsupport_tips,
            data: value.data,
            reply: value.reply,
            enter: value.enter,
        }
    }
}

impl From<Permission> for PermissionWire {
    fn from(value: Permission) -> Self {
        Self {
            kind: value.kind,
            role_ids: value.specify_role_ids,
            user_ids: value.specify_user_ids,
        }
    }
}
