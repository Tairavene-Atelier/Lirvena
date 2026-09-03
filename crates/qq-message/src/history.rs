use prost::Message;

use crate::{
    MessageDecodeError, MessageDecoder, MessageDisposition, MessageEnvelope, RichTextMessage,
    decode_rich_text,
};

/// Linux NT route used to retrieve one bounded group-message interval.
pub const GROUP_HISTORY_ROUTE: &str = "trpc.msg.register_proxy.RegisterProxy.SsoGetGroupMsg";

const MAX_HISTORY_MESSAGES: usize = 100;

/// One decoded historical message and its optional rich-text body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HistoricalMessage {
    envelope: MessageEnvelope,
    rich_text: Option<RichTextMessage>,
}

impl HistoricalMessage {
    /// Returns the authenticated message envelope supplied by QQ.
    #[must_use]
    pub const fn envelope(&self) -> &MessageEnvelope {
        &self.envelope
    }

    /// Returns the decoded rich-text body when present.
    #[must_use]
    pub const fn rich_text(&self) -> Option<&RichTextMessage> {
        self.rich_text.as_ref()
    }

    /// Consumes the historical message into its reusable decoded parts.
    #[must_use]
    pub fn into_parts(self) -> (MessageEnvelope, Option<RichTextMessage>) {
        (self.envelope, self.rich_text)
    }
}

/// Encodes a bounded Linux NT group-history request.
///
/// # Errors
///
/// Returns an error for a missing group, empty interval, reversed interval, or more than 100
/// requested sequence positions.
pub fn encode_group_history_request(
    group_id: u32,
    start_sequence: u32,
    end_sequence: u32,
) -> Result<Vec<u8>, MessageDecodeError> {
    validate_interval(group_id, start_sequence, end_sequence)?;
    Ok(GroupHistoryRequestWire {
        info: Some(GroupHistoryInfoWire {
            group_id,
            start_sequence,
            end_sequence,
        }),
        backwards: true,
    }
    .encode_to_vec())
}

/// Decodes and binds one group-history response to its request interval.
///
/// # Errors
///
/// Returns an error when QQ rejects the request, changes its correlation fields, repeats a
/// message, exceeds the requested bound, or returns a malformed embedded message.
pub fn decode_group_history_response(
    input: &[u8],
    group_id: u32,
    start_sequence: u32,
    end_sequence: u32,
) -> Result<Vec<HistoricalMessage>, MessageDecodeError> {
    validate_interval(group_id, start_sequence, end_sequence)?;
    let response = GroupHistoryResponseWire::decode(input).map_err(|_error| MessageDecodeError)?;
    let body = response.body.ok_or(MessageDecodeError)?;
    if body.result != 0
        || body.group_id != group_id
        || body.start_sequence != start_sequence
        || body.end_sequence != end_sequence
        || body.messages.len() > MAX_HISTORY_MESSAGES
        || body.messages.len() > interval_len(start_sequence, end_sequence)?
    {
        return Err(MessageDecodeError);
    }
    let mut decoder = MessageDecoder::default();
    body.messages
        .into_iter()
        .map(|encoded| {
            let MessageDisposition::New(envelope) = decoder.decode_embedded(&encoded)? else {
                return Err(MessageDecodeError);
            };
            if envelope.class() != crate::MessageClass::Group
                || envelope.route().group_uin != Some(group_id)
                || envelope.sequence() == 0
                || envelope.sequence() < u64::from(start_sequence)
                || envelope.sequence() > u64::from(end_sequence)
            {
                return Err(MessageDecodeError);
            }
            let rich_text = envelope
                .payload()
                .rich_text()
                .map(decode_rich_text)
                .transpose()?;
            Ok(HistoricalMessage {
                envelope: *envelope,
                rich_text,
            })
        })
        .collect()
}

fn validate_interval(group_id: u32, start: u32, end: u32) -> Result<(), MessageDecodeError> {
    if group_id == 0 || end == 0 || start > end || interval_len(start, end)? > MAX_HISTORY_MESSAGES
    {
        return Err(MessageDecodeError);
    }
    Ok(())
}

fn interval_len(start: u32, end: u32) -> Result<usize, MessageDecodeError> {
    let first = start.max(1);
    usize::try_from(u64::from(end) - u64::from(first) + 1).map_err(|_error| MessageDecodeError)
}

#[derive(Clone, PartialEq, Message)]
struct GroupHistoryRequestWire {
    #[prost(message, optional, tag = "1")]
    info: Option<GroupHistoryInfoWire>,
    #[prost(bool, tag = "2")]
    backwards: bool,
}

#[derive(Clone, Copy, PartialEq, Message)]
struct GroupHistoryInfoWire {
    #[prost(uint32, tag = "1")]
    group_id: u32,
    #[prost(uint32, tag = "2")]
    start_sequence: u32,
    #[prost(uint32, tag = "3")]
    end_sequence: u32,
}

#[derive(Clone, PartialEq, Message)]
struct GroupHistoryResponseWire {
    #[prost(message, optional, tag = "3")]
    body: Option<GroupHistoryResponseBodyWire>,
}

#[derive(Clone, PartialEq, Message)]
struct GroupHistoryResponseBodyWire {
    #[prost(uint32, tag = "1")]
    result: u32,
    #[prost(string, tag = "2")]
    message: String,
    #[prost(uint32, tag = "3")]
    group_id: u32,
    #[prost(uint32, tag = "4")]
    start_sequence: u32,
    #[prost(uint32, tag = "5")]
    end_sequence: u32,
    #[prost(bytes = "vec", repeated, tag = "6")]
    messages: Vec<Vec<u8>>,
}
