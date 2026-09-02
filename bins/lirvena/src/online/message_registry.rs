use std::collections::{BTreeMap, VecDeque};

use qq_message::{MessageClass, MessageEnvelope, SendTextTarget};

const MAX_MESSAGES: usize = 4_096;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum RecallTarget {
    Group {
        group_code: u32,
        sequence: u64,
    },
    Private {
        uid: String,
        sequence: u64,
        client_sequence: u64,
        random: u32,
        timestamp: u32,
    },
    Unavailable,
}

pub(super) struct MessageRegistry {
    entries: BTreeMap<u32, RecallTarget>,
    order: VecDeque<u32>,
}

impl MessageRegistry {
    pub(super) const fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
            order: VecDeque::new(),
        }
    }

    pub(super) fn register_inbound(&mut self, envelope: &MessageEnvelope) -> u32 {
        let target = match envelope.class() {
            MessageClass::Group => envelope
                .route()
                .group_uin
                .filter(|group_code| *group_code != 0)
                .map_or(RecallTarget::Unavailable, |group_code| {
                    RecallTarget::Group {
                        group_code,
                        sequence: envelope.sequence(),
                    }
                }),
            MessageClass::Private => private_inbound(envelope),
            _ => RecallTarget::Unavailable,
        };
        self.insert(preferred_id(envelope.sequence()), target)
    }

    pub(super) fn register_outbound(
        &mut self,
        target: &SendTextTarget<'_>,
        sequence: u32,
        client_sequence: u32,
        random: u32,
        timestamp: u32,
    ) -> u32 {
        let target = match target {
            SendTextTarget::Group { group_code } => RecallTarget::Group {
                group_code: *group_code,
                sequence: u64::from(sequence),
            },
            SendTextTarget::Private { uid, .. } => RecallTarget::Private {
                uid: (*uid).to_owned(),
                sequence: u64::from(sequence),
                client_sequence: u64::from(client_sequence),
                random,
                timestamp,
            },
        };
        self.insert(preferred_id(u64::from(sequence)), target)
    }

    pub(super) fn get(&self, message_id: u32) -> Option<&RecallTarget> {
        self.entries.get(&message_id)
    }

    pub(super) fn remove(&mut self, message_id: u32) {
        self.entries.remove(&message_id);
        self.order.retain(|value| *value != message_id);
    }

    fn insert(&mut self, preferred: u32, target: RecallTarget) -> u32 {
        let mut message_id = if preferred == 0 { 1 } else { preferred };
        while self.entries.contains_key(&message_id) {
            message_id = message_id.wrapping_add(1) & 0x7fff_ffff;
            if message_id == 0 {
                message_id = 1;
            }
        }
        self.entries.insert(message_id, target);
        self.order.push_back(message_id);
        while self.entries.len() > MAX_MESSAGES {
            if let Some(expired) = self.order.pop_front() {
                self.entries.remove(&expired);
            }
        }
        message_id
    }
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
    use super::{MessageRegistry, RecallTarget};
    use qq_message::SendTextTarget;

    #[test]
    fn collisions_probe_and_capacity_is_bounded() {
        let mut registry = MessageRegistry::new();
        let target = SendTextTarget::Group { group_code: 42 };
        let first = registry.register_outbound(&target, 7, 8, 9, 10);
        let second = registry.register_outbound(&target, 7, 11, 12, 13);
        assert_eq!(first, 7);
        assert_eq!(second, 8);
        assert!(matches!(
            registry.get(first),
            Some(RecallTarget::Group { .. })
        ));

        for sequence in 100..4_200 {
            registry.register_outbound(&target, sequence, 1, 1, 1);
        }
        assert!(registry.entries.len() <= super::MAX_MESSAGES);
    }
}
