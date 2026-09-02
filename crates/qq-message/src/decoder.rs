use std::collections::{HashSet, VecDeque};

use prost::Message;
use sha2::{Digest, Sha256};

use crate::MessageDecodeError;
use crate::model::{MessageClass, MessageEnvelope, MessageMetadata, MessagePayload, MessageRoute};
use crate::proto::{Push, PushBody, ResponseHead};

const MAX_PUSH_LEN: usize = 1024 * 1024;
const MAX_UID_LEN: usize = 128;
const MAX_DISPLAY_TEXT_LEN: usize = 512;
const MAX_DEDUP_ENTRIES: usize = 2_048;

/// Result of deduplicating and decoding one authenticated message Push.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MessageDisposition {
    /// A duplicate already observed within the bounded current-generation window.
    Duplicate,
    /// A newly admitted outer message envelope.
    New(Box<MessageEnvelope>),
}

/// Bounded current-generation outer message decoder and duplicate window.
#[derive(Debug, Default)]
pub struct MessageDecoder {
    seen: HashSet<DedupKey>,
    order: VecDeque<DedupKey>,
}

impl MessageDecoder {
    /// Decodes a normal outer message Push.
    ///
    /// # Errors
    ///
    /// Returns an error for empty, oversized, malformed or incomplete packets and unsafe text.
    pub fn decode(&mut self, input: &[u8]) -> Result<MessageDisposition, MessageDecodeError> {
        validate_input(input)?;
        let push = Push::decode(input).map_err(|_error| MessageDecodeError)?;
        self.admit(push.message.ok_or(MessageDecodeError)?)
    }

    /// Decodes a message body embedded by a synchronization Push.
    ///
    /// # Errors
    ///
    /// Returns an error for empty, oversized, malformed or incomplete packets and unsafe text.
    pub fn decode_embedded(
        &mut self,
        input: &[u8],
    ) -> Result<MessageDisposition, MessageDecodeError> {
        validate_input(input)?;
        let body = PushBody::decode(input).map_err(|_error| MessageDecodeError)?;
        self.admit(body)
    }

    /// Returns the number of deduplication keys retained for the current generation.
    #[must_use]
    pub fn retained_dedup_entries(&self) -> usize {
        self.seen.len()
    }

    fn admit(&mut self, body: PushBody) -> Result<MessageDisposition, MessageDecodeError> {
        let response = body.response.ok_or(MessageDecodeError)?;
        let content = body.content.ok_or(MessageDecodeError)?;
        validate_response(&response)?;
        validate_content(&content)?;
        let payload = body.body.unwrap_or_default();
        let key = DedupKey::new(
            &response,
            &content,
            payload.content.as_deref().unwrap_or_default(),
        );
        let envelope = MessageEnvelope::new(
            MessageClass::from_wire(content.message_type),
            route(response),
            MessagePayload::new(
                payload.rich_text,
                payload.content,
                payload.encrypted_content,
            ),
            MessageMetadata {
                sub_type: content.sub_type.unwrap_or_default(),
                sequence: content.sequence.unwrap_or_default(),
                random: content.random.unwrap_or_default(),
                timestamp: content.timestamp.unwrap_or_default(),
                package_count: content.package_count.unwrap_or_default(),
                package_index: content.package_index.unwrap_or_default(),
                division_sequence: content.division_sequence.unwrap_or_default(),
                direct_message_sequence: content.direct_message_sequence.unwrap_or_default(),
                message_uid: content.message_uid.unwrap_or_default(),
            },
        );
        if !self.seen.insert(key) {
            return Ok(MessageDisposition::Duplicate);
        }
        self.order.push_back(key);
        if self.order.len() > MAX_DEDUP_ENTRIES {
            let expired = self.order.pop_front().ok_or(MessageDecodeError)?;
            self.seen.remove(&expired);
        }
        Ok(MessageDisposition::New(Box::new(envelope)))
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct DedupKey {
    from_uin: u32,
    message_type: u32,
    sub_type: u32,
    sequence: u64,
    random: i64,
    timestamp: i64,
    package_index: u32,
    content_digest: [u8; 8],
}

impl DedupKey {
    fn new(response: &ResponseHead, content: &crate::proto::ContentHead, body: &[u8]) -> Self {
        let digest = Sha256::digest(body);
        let mut content_digest = [0_u8; 8];
        content_digest.copy_from_slice(&digest[..8]);
        Self {
            from_uin: response.from_uin,
            message_type: content.message_type,
            sub_type: content.sub_type.unwrap_or_default(),
            sequence: content.sequence.unwrap_or_default(),
            random: content.random.unwrap_or_default(),
            timestamp: content.timestamp.unwrap_or_default(),
            package_index: content.package_index.unwrap_or_default(),
            content_digest,
        }
    }
}

fn route(response: ResponseHead) -> MessageRoute {
    let group = response.group.unwrap_or_default();
    MessageRoute {
        from_uin: response.from_uin,
        from_uid: response.from_uid,
        to_uin: response.to_uin,
        to_uid: response.to_uid,
        group_uin: (group.group_uin != 0).then_some(group.group_uin),
        member_name: nonempty(group.member_name),
        group_name: nonempty(group.group_name),
        friend_name: response.forward.and_then(|value| value.friend_name),
    }
}

fn validate_input(input: &[u8]) -> Result<(), MessageDecodeError> {
    if input.is_empty() || input.len() > MAX_PUSH_LEN {
        Err(MessageDecodeError)
    } else {
        Ok(())
    }
}

fn validate_response(response: &ResponseHead) -> Result<(), MessageDecodeError> {
    let valid_uids = response
        .from_uid
        .iter()
        .chain(&response.to_uid)
        .all(|value| valid_text(value, MAX_UID_LEN));
    let valid_forward = response.forward.as_ref().is_none_or(|value| {
        value
            .friend_name
            .as_ref()
            .is_none_or(|text| valid_text(text, MAX_DISPLAY_TEXT_LEN))
    });
    let valid_group = response.group.as_ref().is_none_or(|value| {
        valid_text(&value.member_name, MAX_DISPLAY_TEXT_LEN)
            && valid_text(&value.group_name, MAX_DISPLAY_TEXT_LEN)
    });
    if valid_uids && valid_forward && valid_group {
        Ok(())
    } else {
        Err(MessageDecodeError)
    }
}

fn validate_content(content: &crate::proto::ContentHead) -> Result<(), MessageDecodeError> {
    let valid_forward = content.forward.as_ref().is_none_or(|value| {
        value
            .encoded_value
            .iter()
            .chain(&value.avatar)
            .all(|text| valid_text(text, MAX_DISPLAY_TEXT_LEN))
    });
    if content.message_type != 0 && valid_forward {
        Ok(())
    } else {
        Err(MessageDecodeError)
    }
}

fn valid_text(value: &str, maximum: usize) -> bool {
    value.len() <= maximum && !value.contains('\0')
}

fn nonempty(value: String) -> Option<String> {
    (!value.is_empty()).then_some(value)
}
