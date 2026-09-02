use prost::Message;

use crate::MessageDecodeError;

const MAX_TEXT_BYTES: usize = 4_500;

/// Address of one text-message recipient.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SendTextTarget<'a> {
    /// Direct friend message. The current QQ UID is required by the Linux NT route.
    Private {
        /// Numeric QQ identifier.
        uin: u32,
        /// Current Linux NT UID resolved from the friend directory.
        uid: &'a str,
    },
    /// Group message.
    Group {
        /// Numeric QQ group identifier.
        group_code: u32,
    },
}

/// Bounded input for one ordinary QQ text message.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SendTextInput<'a> {
    /// Recipient routing.
    pub target: SendTextTarget<'a>,
    /// UTF-8 text body.
    pub text: &'a str,
    /// Non-zero local client sequence.
    pub client_sequence: u32,
    /// Non-zero local message random.
    pub random: u32,
    /// Current Unix time used by the private-message control field.
    pub unix_seconds: u32,
}

/// Parsed QQ send acknowledgement.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SendTextOutcome {
    /// QQ result code; zero is success.
    pub result: i32,
    /// Server message sequence.
    pub sequence: u32,
    /// Server timestamp.
    pub timestamp: u32,
}

/// Encodes the tested Linux NT `MessageSvc.PbSendMsg` protobuf body for plain text.
///
/// # Errors
///
/// Returns an error for empty or oversized text, zero correlations, invalid private UID or an
/// out-of-range Unix timestamp.
pub fn encode_text_message(input: &SendTextInput<'_>) -> Result<Vec<u8>, MessageDecodeError> {
    if input.text.is_empty()
        || input.text.len() > MAX_TEXT_BYTES
        || input.client_sequence == 0
        || input.random == 0
    {
        return Err(MessageDecodeError);
    }
    let (routing, private) = match &input.target {
        SendTextTarget::Private { uin, uid }
            if *uin != 0
                && !uid.is_empty()
                && uid.len() <= 128
                && !uid.chars().any(char::is_control) =>
        {
            (
                RoutingHead {
                    c2c: Some(C2c {
                        uin: Some(*uin),
                        uid: Some((*uid).to_owned()),
                    }),
                    group: None,
                },
                true,
            )
        }
        SendTextTarget::Group { group_code } if *group_code != 0 => (
            RoutingHead {
                c2c: None,
                group: Some(Group {
                    group_code: Some(*group_code),
                }),
            },
            false,
        ),
        SendTextTarget::Private { .. } | SendTextTarget::Group { .. } => {
            return Err(MessageDecodeError);
        }
    };
    let control = if private {
        Some(MessageControl {
            message_flag: i32::try_from(input.unix_seconds).map_err(|_error| MessageDecodeError)?,
        })
    } else {
        None
    };
    Ok(MessageWire {
        routing: Some(routing),
        content: Some(ContentHead {
            message_type: 1,
            subtype: Some(0),
            c2c_command: Some(0),
        }),
        body: Some(MessageBody {
            rich_text: Some(RichText {
                elements: vec![Element {
                    text: Some(Text {
                        value: Some(input.text.to_owned()),
                    }),
                }],
            }),
        }),
        client_sequence: Some(input.client_sequence),
        random: Some(input.random),
        control,
    }
    .encode_to_vec())
}

/// Parses one `MessageSvc.PbSendMsg` response.
///
/// # Errors
///
/// Returns an error for malformed protobuf or a successful response without a sequence.
pub fn parse_send_message_response(input: &[u8]) -> Result<SendTextOutcome, MessageDecodeError> {
    let response = SendResponse::decode(input).map_err(|_error| MessageDecodeError)?;
    let sequence = response.group_sequence.unwrap_or(response.private_sequence);
    if response.result == 0 && sequence == 0 {
        return Err(MessageDecodeError);
    }
    Ok(SendTextOutcome {
        result: response.result,
        sequence,
        timestamp: response.timestamp,
    })
}

#[derive(Clone, PartialEq, Message)]
struct MessageWire {
    #[prost(message, optional, tag = "1")]
    routing: Option<RoutingHead>,
    #[prost(message, optional, tag = "2")]
    content: Option<ContentHead>,
    #[prost(message, optional, tag = "3")]
    body: Option<MessageBody>,
    #[prost(uint32, optional, tag = "4")]
    client_sequence: Option<u32>,
    #[prost(uint32, optional, tag = "5")]
    random: Option<u32>,
    #[prost(message, optional, tag = "12")]
    control: Option<MessageControl>,
}

#[derive(Clone, PartialEq, Message)]
struct RoutingHead {
    #[prost(message, optional, tag = "1")]
    c2c: Option<C2c>,
    #[prost(message, optional, tag = "2")]
    group: Option<Group>,
}

#[derive(Clone, PartialEq, Message)]
struct C2c {
    #[prost(uint32, optional, tag = "1")]
    uin: Option<u32>,
    #[prost(string, optional, tag = "2")]
    uid: Option<String>,
}

#[derive(Clone, Copy, PartialEq, Message)]
struct Group {
    #[prost(uint32, optional, tag = "1")]
    group_code: Option<u32>,
}

#[derive(Clone, Copy, PartialEq, Message)]
struct ContentHead {
    #[prost(uint32, tag = "1")]
    message_type: u32,
    #[prost(uint32, optional, tag = "2")]
    subtype: Option<u32>,
    #[prost(uint32, optional, tag = "3")]
    c2c_command: Option<u32>,
}

#[derive(Clone, PartialEq, Message)]
struct MessageBody {
    #[prost(message, optional, tag = "1")]
    rich_text: Option<RichText>,
}

#[derive(Clone, PartialEq, Message)]
struct RichText {
    #[prost(message, repeated, tag = "2")]
    elements: Vec<Element>,
}

#[derive(Clone, PartialEq, Message)]
struct Element {
    #[prost(message, optional, tag = "1")]
    text: Option<Text>,
}

#[derive(Clone, PartialEq, Message)]
struct Text {
    #[prost(string, optional, tag = "1")]
    value: Option<String>,
}

#[derive(Clone, Copy, PartialEq, Message)]
struct MessageControl {
    #[prost(int32, tag = "1")]
    message_flag: i32,
}

#[derive(Clone, PartialEq, Message)]
struct SendResponse {
    #[prost(int32, tag = "1")]
    result: i32,
    #[prost(uint32, tag = "3")]
    timestamp: u32,
    #[prost(uint32, optional, tag = "11")]
    group_sequence: Option<u32>,
    #[prost(uint32, tag = "14")]
    private_sequence: u32,
}
