use std::path::Path;

use account_api::InboundMessage;
use account_message_store::{
    MessageRecord, MessageStore, MessageStoreError, QuoteTarget, RecallTarget,
};
use account_runtime::AccountLocalId;
use adapter_onebot::{IdFormat, project_message_record};
use qq_message::{MessageClass, MessageEnvelope, SendTextTarget};
use serde_json::Value;

pub(super) struct MessageRegistry {
    store: MessageStore,
}

impl MessageRegistry {
    pub(super) fn open(
        directory: &Path,
        local_id: AccountLocalId,
    ) -> Result<Self, MessageStoreError> {
        Ok(Self {
            store: MessageStore::open(directory, local_id)?,
        })
    }

    pub(super) fn prepare_inbound(
        &self,
        envelope: &MessageEnvelope,
    ) -> Result<(u32, RecallTarget), MessageStoreError> {
        let target = match envelope.class() {
            MessageClass::Group => envelope
                .route()
                .group_uin
                .filter(|group_code| *group_code != 0 && envelope.sequence() != 0)
                .map_or(RecallTarget::Unavailable, |group_code| {
                    RecallTarget::Group {
                        group_code,
                        sequence: envelope.sequence(),
                        random: wire_u32(envelope.random()).filter(|value| *value != 0),
                    }
                }),
            MessageClass::Private => private_inbound(envelope),
            _ => RecallTarget::Unavailable,
        };
        Ok((self.next_id(preferred_id(envelope.sequence()))?, target))
    }

    pub(super) fn retain_inbound(
        &mut self,
        message: &InboundMessage,
        recall: RecallTarget,
        inserted_at_ms: u64,
    ) -> Result<(), MessageStoreError> {
        let response = project_message_record(message, IdFormat::Number)
            .map_err(|_error| MessageStoreError::Configuration)?;
        let record = MessageRecord::new(message.message_id(), inserted_at_ms, response, recall)?;
        let record = match inbound_quote(message) {
            Some(quote) => record.with_quote(quote),
            None => record,
        };
        self.store.put(&record)
    }

    pub(super) fn resolve_reply_ids(
        &self,
        envelope: &MessageEnvelope,
        rich_text: Option<&qq_message::RichTextMessage>,
    ) -> Result<Vec<Option<u32>>, MessageStoreError> {
        let Some(rich_text) = rich_text else {
            return Ok(Vec::new());
        };
        rich_text
            .elements()
            .iter()
            .map(|element| {
                let qq_message::Segment::Reply(reply) = element.segment() else {
                    return Ok(None);
                };
                self.store
                    .find_quote(reply.message_uid(), reply.sequence())
                    .map(|record| {
                        record
                            .filter(|record| same_conversation(record.recall(), envelope))
                            .map(|record| record.message_id())
                    })
            })
            .collect()
    }

    pub(super) fn register_outbound(
        &mut self,
        target: &SendTextTarget<'_>,
        correlations: OutboundCorrelations,
        inserted_at_ms: u64,
        mut response: Value,
    ) -> Result<u32, MessageStoreError> {
        let target = match target {
            SendTextTarget::Group { group_code } if correlations.sequence != 0 => {
                RecallTarget::Group {
                    group_code: *group_code,
                    sequence: u64::from(correlations.sequence),
                    random: (correlations.random != 0).then_some(correlations.random),
                }
            }
            SendTextTarget::Private { uid, .. }
                if correlations.sequence != 0
                    && correlations.client_sequence != 0
                    && correlations.random != 0
                    && correlations.timestamp != 0 =>
            {
                RecallTarget::Private {
                    uid: (*uid).to_owned(),
                    sequence: u64::from(correlations.sequence),
                    client_sequence: u64::from(correlations.client_sequence),
                    random: correlations.random,
                    timestamp: correlations.timestamp,
                }
            }
            SendTextTarget::Group { .. } | SendTextTarget::Private { .. } => {
                RecallTarget::Unavailable
            }
        };
        let message_id = self.next_id(preferred_id(u64::from(correlations.sequence)))?;
        let object = response
            .as_object_mut()
            .ok_or(MessageStoreError::Configuration)?;
        object.insert("message_id".to_owned(), Value::from(message_id));
        object.insert("real_id".to_owned(), Value::from(message_id));
        self.store.put(&MessageRecord::new(
            message_id,
            inserted_at_ms,
            response,
            target,
        )?)?;
        Ok(message_id)
    }

    pub(super) fn get(&self, message_id: u32) -> Result<Option<MessageRecord>, MessageStoreError> {
        self.store.get(message_id)
    }

    pub(super) fn remove(&mut self, message_id: u32) -> Result<(), MessageStoreError> {
        self.store.remove(message_id)
    }

    fn next_id(&self, preferred: u32) -> Result<u32, MessageStoreError> {
        let mut message_id = if preferred == 0 { 1 } else { preferred };
        while self.store.contains(message_id)? {
            message_id = message_id.wrapping_add(1) & 0x7fff_ffff;
            if message_id == 0 {
                message_id = 1;
            }
        }
        Ok(message_id)
    }
}

#[derive(Clone, Copy)]
pub(super) struct OutboundCorrelations {
    pub(super) sequence: u32,
    pub(super) client_sequence: u32,
    pub(super) random: u32,
    pub(super) timestamp: u32,
}

fn private_inbound(envelope: &MessageEnvelope) -> RecallTarget {
    let Some(uid) = envelope.route().from_uid.clone() else {
        return RecallTarget::Unavailable;
    };
    let Some(random) = wire_u32(envelope.random()) else {
        return RecallTarget::Unavailable;
    };
    let Ok(timestamp) = u32::try_from(envelope.timestamp()) else {
        return RecallTarget::Unavailable;
    };
    if envelope.sequence() == 0 || envelope.direct_message_sequence() == 0 {
        return RecallTarget::Unavailable;
    }
    RecallTarget::Private {
        uid,
        sequence: envelope.sequence(),
        client_sequence: u64::from(envelope.direct_message_sequence()),
        random,
        timestamp,
    }
}

fn inbound_quote(message: &InboundMessage) -> Option<QuoteTarget> {
    let envelope = message.envelope();
    let sequence = match envelope.class() {
        MessageClass::Group => u32::try_from(envelope.sequence()).ok(),
        MessageClass::Private => Some(envelope.direct_message_sequence()),
        _ => None,
    }?;
    let message_uid = if envelope.message_uid() != 0 {
        envelope.message_uid()
    } else {
        (0x0100_0000_u64 << 32) | u64::from(wire_u32(envelope.random())?)
    };
    let timestamp = u32::try_from(envelope.timestamp()).ok()?;
    let sender_uid = envelope.route().from_uid.clone()?;
    let elements = message
        .rich_text()?
        .elements()
        .iter()
        .map(|element| element.encoded().to_vec())
        .collect();
    QuoteTarget::new(
        sequence,
        message_uid,
        envelope.route().from_uin,
        sender_uid,
        timestamp,
        elements,
    )
    .ok()
}

fn same_conversation(recall: &RecallTarget, envelope: &MessageEnvelope) -> bool {
    match (recall, envelope.class()) {
        (RecallTarget::Group { group_code, .. }, MessageClass::Group) => {
            envelope.route().group_uin == Some(*group_code)
        }
        (RecallTarget::Private { uid, .. }, MessageClass::Private) => {
            envelope.route().from_uid.as_deref() == Some(uid)
        }
        _ => false,
    }
}

fn wire_u32(value: i64) -> Option<u32> {
    u32::try_from(value).ok().or_else(|| {
        i32::try_from(value)
            .ok()
            .map(|signed| u32::from_ne_bytes(signed.to_ne_bytes()))
    })
}

fn preferred_id(sequence: u64) -> u32 {
    let bytes = sequence.to_le_bytes();
    u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) & 0x7fff_ffff
}

#[cfg(test)]
mod tests {
    use account_message_store::RecallTarget;
    use account_runtime::AccountLocalId;
    use qq_message::SendTextTarget;
    use serde_json::json;

    use super::{MessageRegistry, OutboundCorrelations};

    #[test]
    fn outbound_record_survives_runtime_restart() -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let private = directory.path().join("private");
        let local_id = AccountLocalId::from_bytes([11; 16]);
        let mut registry = MessageRegistry::open(&private, local_id)?;
        let message_id = registry.register_outbound(
            &SendTextTarget::Group { group_code: 100 },
            OutboundCorrelations {
                sequence: 90,
                client_sequence: 80,
                random: 70,
                timestamp: 60,
            },
            50,
            json!({"message_type": "group", "message": []}),
        )?;
        drop(registry);

        let reopened = MessageRegistry::open(&private, local_id)?;
        let record = reopened
            .get(message_id)?
            .ok_or_else(|| std::io::Error::other("message was not retained"))?;
        assert_eq!(record.response()["message_id"], json!(message_id));
        assert_eq!(record.response()["real_id"], json!(message_id));
        assert_eq!(
            record.recall(),
            &RecallTarget::Group {
                group_code: 100,
                sequence: 90,
                random: Some(70),
            }
        );
        Ok(())
    }
}
