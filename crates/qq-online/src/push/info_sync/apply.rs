use std::collections::BTreeMap;

use crate::{OnlinePacketError, OnlineSyncState};

use super::proto::{GroupNode, GroupNotification, Packet, PeerActivity, SystemNotification};
use super::state::{
    GroupCursor, InfoSyncPushOutcome, InfoSyncPushState, InfoSyncPushSummary, direct_key,
};
use super::validate::{MAX_EMBEDDED_MESSAGE_LEN, validate_packet};

impl InfoSyncPushState {
    pub(super) fn apply(
        &mut self,
        mut packet: Packet,
        sync: &mut OnlineSyncState,
        available_message_slots: usize,
        available_message_bytes: usize,
        payload_len: usize,
    ) -> Result<InfoSyncPushOutcome, OnlinePacketError> {
        let counts = validate_packet(
            &packet,
            self,
            available_message_slots,
            available_message_bytes,
        )?;
        let group_replay_after_timestamp = sync.group_last_message_time;
        let direct_replay_after_timestamp = sync
            .direct_last_message_time
            .max(sync.previous_direct_message_time);
        let messages = take_messages(&mut packet);
        let summary = InfoSyncPushSummary {
            result: packet.result,
            push_flag: packet.push_flag,
            push_sequence: packet.push_sequence,
            retry_flag: packet.retry_flag,
            use_initial_cache_data: packet.use_initial_cache_data,
            group_node_count: packet.group_nodes.len(),
            group_notification_count: packet
                .group_notifications
                .as_ref()
                .map_or(0, |value| value.items.len()),
            direct_notification_count: packet
                .system_notifications
                .as_ref()
                .map_or(0, |value| value.items.len()),
            embedded_message_count: counts.embedded,
            delivered_message_count: messages.len(),
            delivered_message_bytes: counts.delivered_bytes,
            recent_primary_peer_count: packet
                .recent_activity
                .as_ref()
                .map_or(0, |value| value.primary_peers.len()),
            recent_secondary_peer_count: packet
                .recent_activity
                .as_ref()
                .map_or(0, |value| value.secondary_peers.len()),
            recent_group_count: packet
                .recent_activity
                .as_ref()
                .map_or(0, |value| value.groups.len()),
            roam_message_optimize_flag: packet.roam_message_optimize_flag,
            group_guild_flag: packet.group_guild_flag,
            error_message_len: packet.error_message.len(),
            payload_len,
            auxiliary_state_len: packet.auxiliary_state.as_ref().map_or(0, Vec::len),
            guild_peer_present: packet
                .guild_node
                .as_ref()
                .is_some_and(|value| value.peer_id != 0),
            group_replay_after_timestamp,
            direct_replay_after_timestamp,
        };
        self.apply_group_nodes(packet.group_nodes, sync);
        if let Some(value) = packet.group_notifications {
            self.apply_group_notifications(value.items, sync);
        }
        if let Some(value) = packet.system_notifications {
            self.apply_direct_notifications(value.items, sync);
        }
        if let Some(recent) = packet.recent_activity {
            merge_peer_activity(&mut self.recent_primary, recent.primary_peers);
            merge_peer_activity(&mut self.recent_secondary, recent.secondary_peers);
            for group in recent.groups {
                if group.group_uin != 0 {
                    merge_max(&mut self.recent_groups, group.group_uin, group.timestamp);
                }
            }
        }
        if let Some(auxiliary) = packet.auxiliary_state.filter(|value| !value.is_empty()) {
            self.auxiliary_state = auxiliary;
        }
        if let Some(guild) = packet.guild_node.filter(|value| value.peer_id != 0) {
            self.guild_peer_id = guild.peer_id;
        }
        self.latest = summary;
        Ok(InfoSyncPushOutcome::new(summary, messages))
    }

    fn apply_group_nodes(&mut self, nodes: Vec<GroupNode>, sync: &mut OnlineSyncState) {
        if let Some(latest) = nodes.iter().reduce(|current, candidate| {
            if candidate.latest_message_time > current.latest_message_time {
                candidate
            } else {
                current
            }
        }) && let Ok(group_code) = u32::try_from(latest.group_code)
        {
            sync.last_group_code = group_code;
        }
        for node in nodes {
            sync.group_last_message_time = sync
                .group_last_message_time
                .max(node.latest_message_time.max(node.longest_message_time));
            self.group_cursors
                .entry(node.group_code)
                .or_insert_with(|| GroupCursor::new(node.group_code))
                .merge_node(&node);
        }
    }

    fn apply_group_notifications(
        &mut self,
        notifications: Vec<GroupNotification>,
        sync: &mut OnlineSyncState,
    ) {
        for notification in notifications {
            sync.group_last_message_time = sync
                .group_last_message_time
                .max(notification.last_speak_timestamp);
            self.group_cursors
                .entry(notification.group_code)
                .or_insert_with(|| GroupCursor::new(notification.group_code))
                .merge_notification(&notification);
        }
    }

    fn apply_direct_notifications(
        &mut self,
        notifications: Vec<SystemNotification>,
        sync: &mut OnlineSyncState,
    ) {
        for notification in notifications {
            sync.direct_last_message_time = sync
                .direct_last_message_time
                .max(notification.last_speak_timestamp);
            sync.previous_direct_message_time = sync
                .previous_direct_message_time
                .max(notification.last_speak_timestamp);
            self.direct_cursors
                .entry(direct_key(&notification))
                .or_default()
                .merge(&notification);
        }
    }
}

fn take_messages(packet: &mut Packet) -> Vec<Vec<u8>> {
    let mut output = Vec::new();
    if let Some(value) = &mut packet.group_notifications {
        take_notification_messages(
            &mut output,
            value.items.iter_mut().map(|item| &mut item.messages),
        );
    }
    if let Some(value) = &mut packet.system_notifications {
        take_notification_messages(
            &mut output,
            value.items.iter_mut().map(|item| &mut item.messages),
        );
    }
    output
}

fn take_notification_messages<'a>(
    output: &mut Vec<Vec<u8>>,
    messages: impl Iterator<Item = &'a mut Vec<Vec<u8>>>,
) {
    output.extend(
        messages
            .flat_map(core::mem::take)
            .filter(|message| !message.is_empty() && message.len() <= MAX_EMBEDDED_MESSAGE_LEN),
    );
}

fn merge_peer_activity(target: &mut BTreeMap<String, u64>, peers: Vec<PeerActivity>) {
    for peer in peers {
        if !peer.peer_uid.is_empty() {
            merge_max(target, peer.peer_uid, peer.timestamp);
        }
    }
}

fn merge_max<Key: Ord>(target: &mut BTreeMap<Key, u64>, key: Key, value: u64) {
    let current = target.entry(key).or_default();
    *current = (*current).max(value);
}
