use std::path::Path;

use account_api::{AccountIdentity, InboundMessage};
use account_message_store::{
    MessageRecord, MessageStore, MessageStoreError, QuoteTarget, RecallTarget,
};
use account_runtime::AccountLocalId;
use adapter_onebot::{IdFormat, project_message_record};
use qq_message::{MessageClass, MessageEnvelope, MessageRoute, SendTextTarget};
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

    fn prepare_inbound(
        &self,
        identity: &AccountIdentity,
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
            MessageClass::Private => private_inbound(envelope, identity.qq_id()),
            _ => RecallTarget::Unavailable,
        };
        let message_id = match &target {
            RecallTarget::Group {
                group_code,
                sequence,
                ..
            } => self.store.find_group(*group_code, *sequence)?.map_or_else(
                || self.next_id(preferred_id(envelope.sequence())),
                |record| Ok(record.message_id()),
            )?,
            RecallTarget::Private {
                uid,
                peer_uin: Some(peer_uin),
                sequence,
                client_sequence,
                random,
                timestamp,
            } => self
                .store
                .find_private(
                    uid,
                    *peer_uin,
                    *sequence,
                    *client_sequence,
                    *random,
                    *timestamp,
                )?
                .map_or_else(
                    || self.next_id(preferred_id(envelope.sequence())),
                    |record| Ok(record.message_id()),
                )?,
            RecallTarget::Private { .. } | RecallTarget::Unavailable => {
                self.next_id(preferred_id(envelope.sequence()))?
            }
        };
        Ok((message_id, target))
    }

    pub(super) fn retain_decoded(
        &mut self,
        identity: &AccountIdentity,
        envelope: MessageEnvelope,
        rich_text: Option<qq_message::RichTextMessage>,
        inserted_at_ms: u64,
    ) -> Result<InboundMessage, MessageStoreError> {
        let (message_id, recall) = self.prepare_inbound(identity, &envelope)?;
        let reply_ids = self.resolve_reply_ids(identity, &envelope, rich_text.as_ref())?;
        let message = InboundMessage::new(identity.clone(), message_id, envelope, rich_text)
            .with_reply_ids(reply_ids)
            .map_err(|_error| MessageStoreError::Configuration)?;
        self.retain_inbound(&message, recall, inserted_at_ms)?;
        Ok(message)
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
        identity: &AccountIdentity,
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
                            .filter(|record| {
                                same_conversation(record.recall(), envelope, identity.qq_id())
                            })
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
            SendTextTarget::Private { uin, uid }
                if correlations.sequence != 0
                    && correlations.client_sequence != 0
                    && correlations.random != 0
                    && correlations.timestamp != 0 =>
            {
                RecallTarget::Private {
                    uid: (*uid).to_owned(),
                    peer_uin: Some(*uin),
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

    pub(super) fn find_group_message_id(
        &self,
        group_code: u32,
        sequence: u64,
    ) -> Result<Option<u32>, MessageStoreError> {
        self.store
            .find_group(group_code, sequence)
            .map(|record| record.map(|record| record.message_id()))
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

fn private_inbound(envelope: &MessageEnvelope, self_uin: u64) -> RecallTarget {
    let route = envelope.route();
    let peer = private_peer(route, self_uin);
    let Some((peer_uin, uid)) = peer else {
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
        peer_uin: Some(peer_uin),
        sequence: envelope.sequence(),
        client_sequence: u64::from(envelope.direct_message_sequence()),
        random,
        timestamp,
    }
}

fn private_peer(route: &MessageRoute, self_uin: u64) -> Option<(u32, String)> {
    if u64::from(route.from_uin) == self_uin {
        route
            .to_uid
            .clone()
            .filter(|_uid| route.to_uin != 0)
            .map(|uid| (route.to_uin, uid))
    } else if u64::from(route.to_uin) == self_uin {
        route
            .from_uid
            .clone()
            .filter(|_uid| route.from_uin != 0)
            .map(|uid| (route.from_uin, uid))
    } else {
        None
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

fn same_conversation(recall: &RecallTarget, envelope: &MessageEnvelope, self_uin: u64) -> bool {
    match (recall, envelope.class()) {
        (RecallTarget::Group { group_code, .. }, MessageClass::Group) => {
            envelope.route().group_uin == Some(*group_code)
        }
        (RecallTarget::Private { uid, .. }, MessageClass::Private) => {
            private_peer(envelope.route(), self_uin)
                .is_some_and(|(_peer_uin, peer_uid)| peer_uid == *uid)
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
    use qq_message::{MessageRoute, SendTextTarget};
    use serde_json::json;

    use super::{MessageRegistry, OutboundCorrelations, private_peer};

    #[test]
    fn private_peer_is_selected_for_both_directions() {
        let inbound = MessageRoute {
            from_uin: 42,
            from_uid: Some("u_peer".to_owned()),
            to_uin: 10_001,
            to_uid: Some("u_self".to_owned()),
            ..MessageRoute::default()
        };
        assert_eq!(
            private_peer(&inbound, 10_001),
            Some((42, "u_peer".to_owned()))
        );

        let outbound = MessageRoute {
            from_uin: 10_001,
            from_uid: Some("u_self".to_owned()),
            to_uin: 42,
            to_uid: Some("u_peer".to_owned()),
            ..MessageRoute::default()
        };
        assert_eq!(
            private_peer(&outbound, 10_001),
            Some((42, "u_peer".to_owned()))
        );
        assert_eq!(private_peer(&outbound, 99), None);
    }

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
