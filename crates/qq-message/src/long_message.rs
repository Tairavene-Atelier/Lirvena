use std::io::{Read, Write};

use flate2::Compression;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use prost::Message;

use crate::MessageDecodeError;

const MAX_RESOURCE_ID_BYTES: usize = 512;
const MAX_UID_BYTES: usize = 128;
const MAX_MESSAGES: usize = 100;
const MAX_MESSAGE_BYTES: usize = 1024 * 1024;
const MAX_COMPRESSED_BYTES: usize = 8 * 1024 * 1024;
const MAX_DECOMPRESSED_BYTES: usize = 16 * 1024 * 1024;

/// Recipient used by the Linux NT long-message upload route.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LongMessageTarget<'a> {
    /// One direct friend.
    Private {
        /// Current Linux NT UID of the friend.
        peer_uid: &'a str,
    },
    /// One group.
    Group {
        /// Numeric QQ group identifier.
        group_uin: u32,
    },
}

/// Encodes one Linux NT long-message download request.
///
/// # Errors
///
/// Returns an error for an invalid current-account UID or resource identifier.
pub fn encode_long_message_receive(
    self_uid: &str,
    resource_id: &str,
) -> Result<Vec<u8>, MessageDecodeError> {
    if !valid_uid(self_uid) || !valid_resource_id(resource_id) {
        return Err(MessageDecodeError);
    }
    Ok(LongMessageRequest {
        receive: Some(LongMessageReceiveRequest {
            peer: Some(LongMessagePeer {
                uid: Some(self_uid.to_owned()),
            }),
            resource_id: resource_id.to_owned(),
            message_type: 3,
        }),
        send: None,
        attributes: Some(receive_attributes()),
    }
    .encode_to_vec())
}

/// Encodes one Linux NT long-message upload request.
///
/// The messages are complete encoded QQ common-message bodies.
///
/// # Errors
///
/// Returns an error for invalid routing, empty/excessive messages, oversized
/// content or compression failure.
pub fn encode_long_message_send(
    target: &LongMessageTarget<'_>,
    messages: &[Vec<u8>],
) -> Result<Vec<u8>, MessageDecodeError> {
    let (peer_uid, group_uin, message_type) = match target {
        LongMessageTarget::Private { peer_uid } if valid_uid(peer_uid) => {
            ((*peer_uid).to_owned(), 0, 1)
        }
        LongMessageTarget::Group { group_uin } if *group_uin != 0 => {
            (group_uin.to_string(), *group_uin, 3)
        }
        LongMessageTarget::Private { .. } | LongMessageTarget::Group { .. } => {
            return Err(MessageDecodeError);
        }
    };
    validate_messages(messages)?;
    let content = MultiMessageTransmit {
        messages: Vec::new(),
        items: vec![MultiMessageItem {
            file_name: "MultiMsg".to_owned(),
            buffer: Some(MultiMessageBuffer {
                messages: messages.to_vec(),
            }),
        }],
    }
    .encode_to_vec();
    if content.len() > MAX_DECOMPRESSED_BYTES {
        return Err(MessageDecodeError);
    }
    let payload = compress(&content)?;
    Ok(LongMessageRequest {
        receive: None,
        send: Some(LongMessageSendRequest {
            message_type,
            peer: Some(LongMessagePeer {
                uid: Some(peer_uid),
            }),
            group_uin: u64::from(group_uin),
            payload,
        }),
        attributes: Some(send_attributes()),
    }
    .encode_to_vec())
}

/// Parses one successful long-message download response.
///
/// # Errors
///
/// Returns an error for malformed, missing, ambiguous, compressed-bomb or
/// otherwise out-of-bounds content.
pub fn parse_long_message_receive(input: &[u8]) -> Result<Vec<Vec<u8>>, MessageDecodeError> {
    if input.is_empty() || input.len() > MAX_COMPRESSED_BYTES {
        return Err(MessageDecodeError);
    }
    let response = LongMessageResponse::decode(input).map_err(|_error| MessageDecodeError)?;
    let receive = response.receive.ok_or(MessageDecodeError)?;
    if receive.payload.is_empty() || receive.payload.len() > MAX_COMPRESSED_BYTES {
        return Err(MessageDecodeError);
    }
    let content = decompress(&receive.payload)?;
    let transmit =
        MultiMessageTransmit::decode(content.as_slice()).map_err(|_error| MessageDecodeError)?;
    let mut matching = transmit
        .items
        .into_iter()
        .filter(|item| item.file_name == "MultiMsg");
    let item = matching.next().ok_or(MessageDecodeError)?;
    if matching.next().is_some() {
        return Err(MessageDecodeError);
    }
    let messages = item.buffer.ok_or(MessageDecodeError)?.messages;
    validate_messages(&messages)?;
    Ok(messages)
}

/// Parses one successful long-message upload response.
///
/// # Errors
///
/// Returns an error for malformed data or a missing/invalid resource identifier.
pub fn parse_long_message_send(input: &[u8]) -> Result<String, MessageDecodeError> {
    if input.is_empty() || input.len() > MAX_MESSAGE_BYTES {
        return Err(MessageDecodeError);
    }
    let response = LongMessageResponse::decode(input).map_err(|_error| MessageDecodeError)?;
    let resource_id = response.send.ok_or(MessageDecodeError)?.resource_id;
    if !valid_resource_id(&resource_id) {
        return Err(MessageDecodeError);
    }
    Ok(resource_id)
}

fn validate_messages(messages: &[Vec<u8>]) -> Result<(), MessageDecodeError> {
    if messages.is_empty()
        || messages.len() > MAX_MESSAGES
        || messages
            .iter()
            .any(|message| message.is_empty() || message.len() > MAX_MESSAGE_BYTES)
    {
        return Err(MessageDecodeError);
    }
    let total = messages.iter().try_fold(0usize, |total, message| {
        total.checked_add(message.len()).ok_or(MessageDecodeError)
    })?;
    if total > MAX_DECOMPRESSED_BYTES {
        return Err(MessageDecodeError);
    }
    Ok(())
}

fn compress(input: &[u8]) -> Result<Vec<u8>, MessageDecodeError> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(input)
        .map_err(|_error| MessageDecodeError)?;
    let output = encoder.finish().map_err(|_error| MessageDecodeError)?;
    if output.is_empty() || output.len() > MAX_COMPRESSED_BYTES {
        return Err(MessageDecodeError);
    }
    Ok(output)
}

fn decompress(input: &[u8]) -> Result<Vec<u8>, MessageDecodeError> {
    let decoder = GzDecoder::new(input);
    let limit = u64::try_from(MAX_DECOMPRESSED_BYTES)
        .map_err(|_error| MessageDecodeError)?
        .saturating_add(1);
    let mut output = Vec::new();
    decoder
        .take(limit)
        .read_to_end(&mut output)
        .map_err(|_error| MessageDecodeError)?;
    if output.is_empty() || output.len() > MAX_DECOMPRESSED_BYTES {
        return Err(MessageDecodeError);
    }
    Ok(output)
}

fn valid_uid(value: &str) -> bool {
    !value.is_empty() && value.len() <= MAX_UID_BYTES && !value.chars().any(char::is_control)
}

pub(crate) fn valid_resource_id(value: &str) -> bool {
    !value.is_empty()
        && !value.trim().is_empty()
        && value.len() <= MAX_RESOURCE_ID_BYTES
        && !value.chars().any(char::is_control)
}

const fn receive_attributes() -> LongMessageAttributes {
    LongMessageAttributes {
        subcommand: 2,
        client_type: Some(0),
        platform: Some(0),
        proxy_type: Some(0),
    }
}

const fn send_attributes() -> LongMessageAttributes {
    LongMessageAttributes {
        subcommand: 4,
        client_type: Some(1),
        platform: Some(6),
        proxy_type: Some(0),
    }
}

#[derive(Clone, PartialEq, Message)]
struct MultiMessageTransmit {
    #[prost(bytes = "vec", repeated, tag = "1")]
    messages: Vec<Vec<u8>>,
    #[prost(message, repeated, tag = "2")]
    items: Vec<MultiMessageItem>,
}

#[derive(Clone, PartialEq, Message)]
struct MultiMessageItem {
    #[prost(string, tag = "1")]
    file_name: String,
    #[prost(message, optional, tag = "2")]
    buffer: Option<MultiMessageBuffer>,
}

#[derive(Clone, PartialEq, Message)]
struct MultiMessageBuffer {
    #[prost(bytes = "vec", repeated, tag = "1")]
    messages: Vec<Vec<u8>>,
}

#[derive(Clone, PartialEq, Message)]
struct LongMessageRequest {
    #[prost(message, optional, tag = "1")]
    receive: Option<LongMessageReceiveRequest>,
    #[prost(message, optional, tag = "2")]
    send: Option<LongMessageSendRequest>,
    #[prost(message, optional, tag = "15")]
    attributes: Option<LongMessageAttributes>,
}

#[derive(Clone, PartialEq, Message)]
struct LongMessageResponse {
    #[prost(message, optional, tag = "1")]
    receive: Option<LongMessageReceiveResponse>,
    #[prost(message, optional, tag = "2")]
    send: Option<LongMessageSendResponse>,
}

#[derive(Clone, PartialEq, Message)]
struct LongMessageAttributes {
    #[prost(uint32, tag = "1")]
    subcommand: u32,
    #[prost(uint32, optional, tag = "2")]
    client_type: Option<u32>,
    #[prost(uint32, optional, tag = "3")]
    platform: Option<u32>,
    #[prost(uint32, optional, tag = "4")]
    proxy_type: Option<u32>,
}

#[derive(Clone, PartialEq, Message)]
struct LongMessagePeer {
    #[prost(string, optional, tag = "2")]
    uid: Option<String>,
}

#[derive(Clone, PartialEq, Message)]
struct LongMessageReceiveRequest {
    #[prost(message, optional, tag = "1")]
    peer: Option<LongMessagePeer>,
    #[prost(string, tag = "2")]
    resource_id: String,
    #[prost(uint32, tag = "3")]
    message_type: u32,
}

#[derive(Clone, PartialEq, Message)]
struct LongMessageSendRequest {
    #[prost(uint32, tag = "1")]
    message_type: u32,
    #[prost(message, optional, tag = "2")]
    peer: Option<LongMessagePeer>,
    #[prost(uint64, tag = "3")]
    group_uin: u64,
    #[prost(bytes = "vec", tag = "4")]
    payload: Vec<u8>,
}

#[derive(Clone, PartialEq, Message)]
struct LongMessageSendResponse {
    #[prost(string, tag = "3")]
    resource_id: String,
}

#[derive(Clone, PartialEq, Message)]
struct LongMessageReceiveResponse {
    #[prost(bytes = "vec", tag = "4")]
    payload: Vec<u8>,
}
