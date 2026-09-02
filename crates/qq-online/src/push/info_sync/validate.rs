use std::collections::{BTreeMap, BTreeSet};

use crate::OnlinePacketError;

use super::proto::Packet;
use super::state::{InfoSyncPushState, direct_key};

const MAX_COLLECTION_ITEMS: usize = 4_096;
const MAX_PERSISTENT_CURSORS: usize = 8_192;
pub(super) const MAX_EMBEDDED_MESSAGE_LEN: usize = 1024 * 1024;
const MAX_AUXILIARY_STATE_LEN: usize = 2 * 1024 * 1024;
const MAX_ERROR_TEXT_LEN: usize = 64 * 1024;
const MAX_PEER_TEXT_LEN: usize = 512;

pub(super) struct ValidatedCounts {
    pub(super) embedded: usize,
    pub(super) delivered_bytes: usize,
}

pub(super) fn validate_packet(
    packet: &Packet,
    state: &InfoSyncPushState,
    available_message_slots: usize,
    available_message_bytes: usize,
) -> Result<ValidatedCounts, OnlinePacketError> {
    if packet.error_message.len() > MAX_ERROR_TEXT_LEN
        || packet.group_nodes.len() > MAX_COLLECTION_ITEMS
        || packet
            .group_notifications
            .as_ref()
            .is_some_and(|value| value.items.len() > MAX_COLLECTION_ITEMS)
        || packet
            .system_notifications
            .as_ref()
            .is_some_and(|value| value.items.len() > MAX_COLLECTION_ITEMS)
        || packet
            .auxiliary_state
            .as_ref()
            .is_some_and(|value| value.len() > MAX_AUXILIARY_STATE_LEN)
    {
        return Err(OnlinePacketError);
    }
    validate_recent(packet)?;
    validate_persistent_capacity(packet, state)?;
    let (embedded, deliverable, delivered_bytes) = message_counts(packet)?;
    if deliverable > MAX_COLLECTION_ITEMS
        || deliverable > available_message_slots
        || delivered_bytes > available_message_bytes
    {
        return Err(OnlinePacketError);
    }
    Ok(ValidatedCounts {
        embedded,
        delivered_bytes,
    })
}

fn validate_recent(packet: &Packet) -> Result<(), OnlinePacketError> {
    let Some(recent) = &packet.recent_activity else {
        return Ok(());
    };
    if recent.primary_peers.len() > MAX_COLLECTION_ITEMS
        || recent.secondary_peers.len() > MAX_COLLECTION_ITEMS
        || recent.groups.len() > MAX_COLLECTION_ITEMS
        || recent
            .primary_peers
            .iter()
            .chain(&recent.secondary_peers)
            .any(|peer| peer.peer_uid.len() > MAX_PEER_TEXT_LEN)
    {
        return Err(OnlinePacketError);
    }
    Ok(())
}

fn validate_persistent_capacity(
    packet: &Packet,
    state: &InfoSyncPushState,
) -> Result<(), OnlinePacketError> {
    let mut groups = BTreeSet::new();
    groups.extend(packet.group_nodes.iter().map(|node| node.group_code));
    if let Some(value) = &packet.group_notifications {
        groups.extend(value.items.iter().map(|item| item.group_code));
    }
    let new_groups = groups
        .iter()
        .filter(|key| !state.group_cursors.contains_key(key))
        .count();
    let mut directs = BTreeSet::new();
    if let Some(value) = &packet.system_notifications {
        for item in &value.items {
            if item.peer_uid.len() > MAX_PEER_TEXT_LEN {
                return Err(OnlinePacketError);
            }
            directs.insert(direct_key(item));
        }
    }
    let new_directs = directs
        .iter()
        .filter(|key| !state.direct_cursors.contains_key(*key))
        .count();
    if state.group_cursors.len().saturating_add(new_groups) > MAX_PERSISTENT_CURSORS
        || state.direct_cursors.len().saturating_add(new_directs) > MAX_PERSISTENT_CURSORS
        || projected_recent_len(&state.recent_primary, packet, true) > MAX_PERSISTENT_CURSORS
        || projected_recent_len(&state.recent_secondary, packet, false) > MAX_PERSISTENT_CURSORS
        || projected_recent_group_len(&state.recent_groups, packet) > MAX_PERSISTENT_CURSORS
    {
        return Err(OnlinePacketError);
    }
    Ok(())
}

fn projected_recent_len(existing: &BTreeMap<String, u64>, packet: &Packet, primary: bool) -> usize {
    let mut keys = BTreeSet::new();
    if let Some(recent) = &packet.recent_activity {
        let peers = if primary {
            &recent.primary_peers
        } else {
            &recent.secondary_peers
        };
        keys.extend(
            peers
                .iter()
                .filter(|peer| !peer.peer_uid.is_empty())
                .map(|peer| peer.peer_uid.as_str()),
        );
    }
    existing.len()
        + keys
            .iter()
            .filter(|key| !existing.contains_key(**key))
            .count()
}

fn projected_recent_group_len(existing: &BTreeMap<u64, u64>, packet: &Packet) -> usize {
    let mut keys = BTreeSet::new();
    if let Some(recent) = &packet.recent_activity {
        keys.extend(
            recent
                .groups
                .iter()
                .filter(|group| group.group_uin != 0)
                .map(|group| group.group_uin),
        );
    }
    existing.len()
        + keys
            .iter()
            .filter(|key| !existing.contains_key(key))
            .count()
}

fn message_counts(packet: &Packet) -> Result<(usize, usize, usize), OnlinePacketError> {
    let messages = packet
        .group_notifications
        .iter()
        .flat_map(|value| &value.items)
        .flat_map(|item| &item.messages)
        .chain(
            packet
                .system_notifications
                .iter()
                .flat_map(|value| &value.items)
                .flat_map(|item| &item.messages),
        );
    let mut embedded = 0_usize;
    let mut deliverable = 0_usize;
    let mut delivered_bytes = 0_usize;
    for message in messages {
        embedded = embedded.checked_add(1).ok_or(OnlinePacketError)?;
        if !message.is_empty() && message.len() <= MAX_EMBEDDED_MESSAGE_LEN {
            deliverable = deliverable.checked_add(1).ok_or(OnlinePacketError)?;
            delivered_bytes = delivered_bytes
                .checked_add(message.len())
                .ok_or(OnlinePacketError)?;
        }
    }
    Ok((embedded, deliverable, delivered_bytes))
}
