use std::collections::BTreeMap;

use super::proto::{GroupNode, GroupNotification, SystemNotification};

/// Bounded diagnostic summary of the latest synchronization Push.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct InfoSyncPushSummary {
    /// Remote result value.
    pub result: u32,
    /// Remote Push flags.
    pub push_flag: u32,
    /// Remote Push sequence.
    pub push_sequence: u32,
    /// Remote retry flag.
    pub retry_flag: u32,
    /// Remote initial-cache selector.
    pub use_initial_cache_data: u32,
    /// Number of group cursor nodes.
    pub group_node_count: usize,
    /// Number of group notification nodes.
    pub group_notification_count: usize,
    /// Number of direct notification nodes.
    pub direct_notification_count: usize,
    /// Number of embedded message bodies present before delivery filtering.
    pub embedded_message_count: usize,
    /// Number of valid embedded message bodies selected for delivery.
    pub delivered_message_count: usize,
    /// Total byte length of valid embedded message bodies selected for delivery.
    pub delivered_message_bytes: usize,
    /// Number of recent primary peers.
    pub recent_primary_peer_count: usize,
    /// Number of recent secondary peers.
    pub recent_secondary_peer_count: usize,
    /// Number of recent groups.
    pub recent_group_count: usize,
    /// Remote roam-message optimization flag.
    pub roam_message_optimize_flag: u32,
    /// Remote group-guild flag.
    pub group_guild_flag: u32,
    /// Bounded error-text byte length without retaining the text.
    pub error_message_len: usize,
    /// Authenticated Push payload byte length.
    pub payload_len: usize,
    /// Auxiliary-state byte length carried by this Push.
    pub auxiliary_state_len: usize,
    /// Whether this Push carried a non-zero guild peer identifier.
    pub guild_peer_present: bool,
    /// Group replay baseline captured before this Push was applied.
    pub group_replay_after_timestamp: u64,
    /// Direct replay baseline captured before this Push was applied.
    pub direct_replay_after_timestamp: u64,
}

/// Bounded result of applying one synchronization Push.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InfoSyncPushOutcome {
    summary: InfoSyncPushSummary,
    messages: Vec<Vec<u8>>,
}

impl InfoSyncPushOutcome {
    pub(super) const fn new(summary: InfoSyncPushSummary, messages: Vec<Vec<u8>>) -> Self {
        Self { summary, messages }
    }

    /// Returns the bounded diagnostic summary.
    #[must_use]
    pub const fn summary(&self) -> InfoSyncPushSummary {
        self.summary
    }

    /// Consumes the outcome and returns authenticated embedded message bodies in wire order.
    #[must_use]
    pub fn into_messages(self) -> Vec<Vec<u8>> {
        self.messages
    }
}

/// Bounded per-generation state retained from synchronization Push packets.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct InfoSyncPushState {
    pub(super) latest: InfoSyncPushSummary,
    pub(super) group_cursors: BTreeMap<u64, GroupCursor>,
    pub(super) direct_cursors: BTreeMap<String, DirectCursor>,
    pub(super) recent_primary: BTreeMap<String, u64>,
    pub(super) recent_secondary: BTreeMap<String, u64>,
    pub(super) recent_groups: BTreeMap<u64, u64>,
    pub(super) auxiliary_state: Vec<u8>,
    pub(super) guild_peer_id: u64,
}

impl InfoSyncPushState {
    /// Returns the latest bounded summary.
    #[must_use]
    pub const fn latest(&self) -> InfoSyncPushSummary {
        self.latest
    }

    /// Returns the number of retained group cursors.
    #[must_use]
    pub fn group_cursor_count(&self) -> usize {
        self.group_cursors.len()
    }

    /// Returns the number of retained direct-message cursors.
    #[must_use]
    pub fn direct_cursor_count(&self) -> usize {
        self.direct_cursors.len()
    }

    /// Returns the current auxiliary-state byte length.
    #[must_use]
    pub fn auxiliary_state_len(&self) -> usize {
        self.auxiliary_state.len()
    }

    /// Returns the latest non-zero guild peer identifier.
    #[must_use]
    pub const fn guild_peer_id(&self) -> u64 {
        self.guild_peer_id
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct GroupCursor {
    group_code: u64,
    group_sequence: u64,
    read_message_sequence: u64,
    mask: u64,
    longest_message_time: u64,
    latest_message_time: u64,
    longest_message_sequence: u64,
    important_message_latest_sequence: u32,
    group_max_event_sequence: u32,
    has_message: bool,
    need_check_sequence_on_aio_open: u32,
    notification_start_sequence: u32,
    notification_end_sequence: u32,
    last_speak_timestamp: u64,
}

impl GroupCursor {
    pub(super) const fn new(group_code: u64) -> Self {
        Self {
            group_code,
            group_sequence: 0,
            read_message_sequence: 0,
            mask: 0,
            longest_message_time: 0,
            latest_message_time: 0,
            longest_message_sequence: 0,
            important_message_latest_sequence: 0,
            group_max_event_sequence: 0,
            has_message: false,
            need_check_sequence_on_aio_open: 0,
            notification_start_sequence: 0,
            notification_end_sequence: 0,
            last_speak_timestamp: 0,
        }
    }

    pub(super) fn merge_node(&mut self, node: &GroupNode) {
        self.group_sequence = self.group_sequence.max(node.group_sequence);
        self.read_message_sequence = self.read_message_sequence.max(node.read_message_sequence);
        self.mask = node.mask;
        self.longest_message_time = self.longest_message_time.max(node.longest_message_time);
        self.latest_message_time = self.latest_message_time.max(node.latest_message_time);
        self.longest_message_sequence = self
            .longest_message_sequence
            .max(node.longest_message_sequence);
        self.important_message_latest_sequence = self
            .important_message_latest_sequence
            .max(node.important_message_latest_sequence);
        self.group_max_event_sequence = self
            .group_max_event_sequence
            .max(node.group_max_event_sequence);
        self.has_message |= node.has_message;
        self.need_check_sequence_on_aio_open = node.need_check_sequence_on_aio_open;
    }

    pub(super) fn merge_notification(&mut self, notification: &GroupNotification) {
        self.notification_start_sequence = if self.notification_start_sequence == 0 {
            notification.start_sequence
        } else {
            self.notification_start_sequence
                .min(notification.start_sequence)
        };
        self.notification_end_sequence = self
            .notification_end_sequence
            .max(notification.end_sequence);
        self.last_speak_timestamp = self
            .last_speak_timestamp
            .max(notification.last_speak_timestamp);
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct DirectCursor {
    peer_uin: u64,
    peer_uid: String,
    last_speak_timestamp: u64,
}

impl DirectCursor {
    pub(super) fn merge(&mut self, notification: &SystemNotification) {
        if self.peer_uin == 0 && self.peer_uid.is_empty() {
            self.peer_uin = notification.peer_uin;
            self.peer_uid.clone_from(&notification.peer_uid);
        }
        self.last_speak_timestamp = self
            .last_speak_timestamp
            .max(notification.last_speak_timestamp);
    }
}

pub(super) fn direct_key(notification: &SystemNotification) -> String {
    if notification.peer_uid.is_empty() {
        notification.peer_uin.to_string()
    } else {
        notification.peer_uid.clone()
    }
}
