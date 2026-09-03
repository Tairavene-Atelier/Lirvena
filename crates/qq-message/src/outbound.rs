use prost::Message;

use crate::MessageDecodeError;

const MAX_TEXT_BYTES: usize = 4_500;
const MAX_ELEMENTS: usize = 256;
const MAX_DISPLAY_BYTES: usize = 1_024;
const MAX_MEDIA_METADATA_BYTES: usize = 1024 * 1024;
const MAX_MEDIA_COMPATIBILITY_BYTES: usize = 512 * 1024;

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

/// One compiled outbound message element.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OutboundSegment<'a> {
    /// Plain UTF-8 text.
    Text(&'a str),
    /// Mention every member in a group.
    MentionEveryone {
        /// Human-readable preview carried by QQ.
        display: &'a str,
    },
    /// Mention one resolved group member.
    Mention {
        /// Numeric QQ identifier.
        uin: u32,
        /// Current Linux NT UID.
        uid: &'a str,
        /// Human-readable preview carried by QQ.
        display: &'a str,
    },
    /// One classic QQ face identifier.
    Face(u16),
    /// Tencent-created modern and legacy image message material.
    Image {
        /// Whether this image targets a group scene.
        group: bool,
        /// Opaque modern message information returned by QQ.
        message_info: &'a [u8],
        /// Opaque legacy compatibility message returned by QQ.
        compatibility: &'a [u8],
    },
    /// Tencent-created modern voice-message material.
    Record {
        /// Whether this record targets a group scene.
        group: bool,
        /// Opaque modern message information returned by QQ.
        message_info: &'a [u8],
    },
    /// Tencent-created modern video material and optional legacy compatibility material.
    Video {
        /// Whether this video targets a group scene.
        group: bool,
        /// Opaque modern message information returned by QQ.
        message_info: &'a [u8],
        /// Opaque legacy video element returned by QQ.
        compatibility: &'a [u8],
    },
    /// One compressed light-application JSON payload.
    Json(&'a str),
    /// One compressed XML rich-message payload.
    Xml {
        /// XML body.
        body: &'a str,
        /// QQ rich-message service identifier.
        service_id: i32,
    },
    /// One QQ shake/poke element.
    Poke {
        /// Poke kind.
        kind: u32,
        /// Poke strength.
        strength: u32,
    },
}

/// Bounded input for one ordinary QQ rich-text message.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SendMessageInput<'a> {
    /// Recipient routing.
    pub target: SendTextTarget<'a>,
    /// Already resolved outbound elements.
    pub segments: &'a [OutboundSegment<'a>],
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
    encode_message(&SendMessageInput {
        target: input.target.clone(),
        segments: &[OutboundSegment::Text(input.text)],
        client_sequence: input.client_sequence,
        random: input.random,
        unix_seconds: input.unix_seconds,
    })
}

/// Encodes the tested Linux NT `MessageSvc.PbSendMsg` protobuf body.
///
/// # Errors
///
/// Returns an error for invalid routing, empty/excessive elements, unresolved mentions, unsupported
/// face IDs, zero correlations, or an out-of-range Unix timestamp.
pub fn encode_message(input: &SendMessageInput<'_>) -> Result<Vec<u8>, MessageDecodeError> {
    if input.segments.is_empty()
        || input.segments.len() > MAX_ELEMENTS
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
    let mut text_bytes = 0usize;
    let mut elements = Vec::with_capacity(input.segments.len() * 2);
    for segment in input.segments {
        let produced = match segment {
            OutboundSegment::Text(value) if !value.is_empty() => {
                text_bytes = text_bytes
                    .checked_add(value.len())
                    .ok_or(MessageDecodeError)?;
                Ok(vec![Element {
                    text: Some(Text {
                        value: Some((*value).to_owned()),
                        reserve: None,
                    }),
                    ..Element::default()
                }])
            }
            OutboundSegment::MentionEveryone { display } if valid_display(display) => {
                text_bytes = text_bytes
                    .checked_add(display.len())
                    .ok_or(MessageDecodeError)?;
                Ok(vec![mention_element(display, 1, 0, "")])
            }
            OutboundSegment::Mention { uin, uid, display }
                if *uin != 0 && valid_uid(uid) && valid_display(display) =>
            {
                text_bytes = text_bytes
                    .checked_add(display.len())
                    .ok_or(MessageDecodeError)?;
                Ok(vec![mention_element(display, 2, *uin, uid)])
            }
            OutboundSegment::Face(id) if *id < 260 => Ok(vec![Element {
                face: Some(Face {
                    index: Some(i32::from(*id)),
                }),
                ..Element::default()
            }]),
            OutboundSegment::Image {
                group,
                message_info,
                compatibility,
            } if !message_info.is_empty()
                && message_info.len() <= MAX_MEDIA_METADATA_BYTES
                && compatibility.len() <= MAX_MEDIA_COMPATIBILITY_BYTES =>
            {
                Ok(image_elements(*group, message_info, compatibility))
            }
            OutboundSegment::Record {
                group,
                message_info,
            } if !message_info.is_empty() && message_info.len() <= MAX_MEDIA_METADATA_BYTES => {
                Ok(vec![common_media_element(
                    if *group { 22 } else { 12 },
                    message_info,
                )])
            }
            OutboundSegment::Video {
                group,
                message_info,
                compatibility,
            } if !message_info.is_empty()
                && message_info.len() <= MAX_MEDIA_METADATA_BYTES
                && compatibility.len() <= MAX_MEDIA_COMPATIBILITY_BYTES =>
            {
                Ok(video_elements(*group, message_info, compatibility))
            }
            OutboundSegment::Json(body) => Ok(vec![Element {
                light_app: Some(LightApp {
                    data: crate::rich_content::compress(body)?,
                    resource_id: None,
                }),
                ..Element::default()
            }]),
            OutboundSegment::Xml { body, service_id } if *service_id > 0 => Ok(vec![Element {
                rich_message: Some(RichMessage {
                    template: crate::rich_content::compress(body)?,
                    service_id: Some(*service_id),
                }),
                ..Element::default()
            }]),
            OutboundSegment::Poke { kind, strength } if *kind != 0 => Ok(vec![common_element(
                2,
                *kind,
                crate::rich_content::encode_poke(*kind, *strength),
            )]),
            OutboundSegment::Text(_)
            | OutboundSegment::MentionEveryone { .. }
            | OutboundSegment::Mention { .. }
            | OutboundSegment::Face(_)
            | OutboundSegment::Image { .. }
            | OutboundSegment::Record { .. }
            | OutboundSegment::Video { .. }
            | OutboundSegment::Xml { .. }
            | OutboundSegment::Poke { .. } => Err(MessageDecodeError),
        }?;
        elements.extend(produced);
    }
    if text_bytes > MAX_TEXT_BYTES {
        return Err(MessageDecodeError);
    }
    Ok(MessageWire {
        routing: Some(routing),
        content: Some(ContentHead {
            message_type: 1,
            subtype: Some(0),
            c2c_command: Some(0),
        }),
        body: Some(MessageBody {
            rich_text: Some(RichText { elements }),
        }),
        client_sequence: Some(input.client_sequence),
        random: Some(input.random),
        control,
    }
    .encode_to_vec())
}

fn mention_element(display: &str, kind: i32, uin: u32, uid: &str) -> Element {
    Element {
        text: Some(Text {
            value: Some(display.to_owned()),
            reserve: Some(
                MentionExtra {
                    kind: Some(kind),
                    uin: Some(uin),
                    field5: Some(0),
                    uid: Some(uid.to_owned()),
                }
                .encode_to_vec(),
            ),
        }),
        ..Element::default()
    }
}

fn image_elements(group: bool, message_info: &[u8], compatibility: &[u8]) -> Vec<Element> {
    let common = common_media_element(if group { 20 } else { 10 }, message_info);
    if compatibility.is_empty() {
        return vec![common];
    }
    let (not_online_image, custom_face) = if group {
        (None, Some(compatibility.to_vec()))
    } else {
        (Some(compatibility.to_vec()), None)
    };
    vec![
        Element {
            not_online_image,
            custom_face,
            ..Element::default()
        },
        common,
    ]
}

fn common_media_element(business_type: u32, message_info: &[u8]) -> Element {
    common_element(48, business_type, message_info.to_vec())
}

fn common_element(service_type: i32, business_type: u32, protobuf: Vec<u8>) -> Element {
    Element {
        common: Some(CommonElement {
            service_type,
            protobuf,
            business_type,
        }),
        ..Element::default()
    }
}

fn video_elements(group: bool, message_info: &[u8], compatibility: &[u8]) -> Vec<Element> {
    let common = common_media_element(if group { 21 } else { 11 }, message_info);
    if compatibility.is_empty() {
        return vec![common];
    }
    vec![
        Element {
            video: Some(compatibility.to_vec()),
            ..Element::default()
        },
        common,
    ]
}

fn valid_uid(value: &str) -> bool {
    !value.is_empty() && value.len() <= 128 && !value.chars().any(char::is_control)
}

fn valid_display(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_DISPLAY_BYTES
        && !value.chars().any(|character| character == '\0')
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
    #[prost(message, optional, tag = "2")]
    face: Option<Face>,
    #[prost(bytes = "vec", optional, tag = "4")]
    not_online_image: Option<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "8")]
    custom_face: Option<Vec<u8>>,
    #[prost(message, optional, tag = "12")]
    rich_message: Option<RichMessage>,
    #[prost(bytes = "vec", optional, tag = "19")]
    video: Option<Vec<u8>>,
    #[prost(message, optional, tag = "51")]
    light_app: Option<LightApp>,
    #[prost(message, optional, tag = "53")]
    common: Option<CommonElement>,
}

#[derive(Clone, PartialEq, Message)]
struct RichMessage {
    #[prost(bytes = "vec", tag = "1")]
    template: Vec<u8>,
    #[prost(int32, optional, tag = "2")]
    service_id: Option<i32>,
}

#[derive(Clone, PartialEq, Message)]
struct LightApp {
    #[prost(bytes = "vec", tag = "1")]
    data: Vec<u8>,
    #[prost(bytes = "vec", optional, tag = "2")]
    resource_id: Option<Vec<u8>>,
}

#[derive(Clone, PartialEq, Message)]
struct CommonElement {
    #[prost(int32, tag = "1")]
    service_type: i32,
    #[prost(bytes = "vec", tag = "2")]
    protobuf: Vec<u8>,
    #[prost(uint32, tag = "3")]
    business_type: u32,
}

#[derive(Clone, PartialEq, Message)]
struct Text {
    #[prost(string, optional, tag = "1")]
    value: Option<String>,
    #[prost(bytes = "vec", optional, tag = "12")]
    reserve: Option<Vec<u8>>,
}

#[derive(Clone, Copy, PartialEq, Message)]
struct Face {
    #[prost(int32, optional, tag = "1")]
    index: Option<i32>,
}

#[derive(Clone, PartialEq, Message)]
struct MentionExtra {
    #[prost(int32, optional, tag = "3")]
    kind: Option<i32>,
    #[prost(uint32, optional, tag = "4")]
    uin: Option<u32>,
    #[prost(int32, optional, tag = "5")]
    field5: Option<i32>,
    #[prost(string, optional, tag = "9")]
    uid: Option<String>,
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
