mod apply;
mod proto;
mod state;
mod validate;

pub use state::{InfoSyncPushOutcome, InfoSyncPushState, InfoSyncPushSummary};

use prost::Message;

use crate::{OnlinePacketError, OnlineSyncState};

const MAX_BODY_LEN: usize = 2 * 1024 * 1024;

pub(super) fn apply(
    body: &[u8],
    state: &mut InfoSyncPushState,
    sync: &mut OnlineSyncState,
    available_message_slots: usize,
    available_message_bytes: usize,
) -> Result<InfoSyncPushOutcome, OnlinePacketError> {
    if body.is_empty() || body.len() > MAX_BODY_LEN {
        return Err(OnlinePacketError);
    }
    let packet = proto::Packet::decode(body).map_err(|_error| OnlinePacketError)?;
    state.apply(
        packet,
        sync,
        available_message_slots,
        available_message_bytes,
        body.len(),
    )
}

#[cfg(test)]
mod tests {
    use prost::Message;

    use super::proto::{
        GroupActivity, GroupNode, GroupNotification, GroupNotifications, GuildNode, Packet,
        PeerActivity, RecentActivity, SystemNotification, SystemNotifications,
    };
    use super::{InfoSyncPushState, apply};
    use crate::OnlineSyncState;

    #[test]
    fn applies_monotonic_cursors_and_preserves_message_order()
    -> Result<(), crate::OnlinePacketError> {
        let mut state = InfoSyncPushState::default();
        let mut sync = OnlineSyncState {
            group_last_message_time: 10,
            direct_last_message_time: 20,
            previous_direct_message_time: 30,
            ..OnlineSyncState::default()
        };
        let packet = fixture();
        let outcome = apply(&packet.encode_to_vec(), &mut state, &mut sync, 8, 1024)?;
        let summary = outcome.summary();
        assert_eq!(summary.group_replay_after_timestamp, 10);
        assert_eq!(summary.direct_replay_after_timestamp, 30);
        assert_eq!(summary.embedded_message_count, 2);
        assert_eq!(summary.delivered_message_count, 2);
        assert_eq!(summary.delivered_message_bytes, 4);
        assert_eq!(summary.recent_primary_peer_count, 1);
        assert_eq!(summary.recent_group_count, 1);
        assert_eq!(summary.roam_message_optimize_flag, 5);
        assert_eq!(summary.group_guild_flag, 6);
        assert!(summary.guild_peer_present);
        assert_eq!(outcome.into_messages(), vec![vec![1, 2], vec![3, 4]]);
        assert_eq!(sync.group_last_message_time, 44);
        assert_eq!(sync.direct_last_message_time, 55);
        assert_eq!(sync.previous_direct_message_time, 55);
        assert_eq!(sync.last_group_code, 42);
        assert_eq!(state.group_cursor_count(), 1);
        assert_eq!(state.direct_cursor_count(), 1);
        assert_eq!(state.auxiliary_state_len(), 3);
        assert_eq!(state.guild_peer_id(), 77);
        Ok(())
    }

    #[test]
    fn insufficient_queue_capacity_is_transactional() {
        let mut state = InfoSyncPushState::default();
        let mut sync = OnlineSyncState::default();
        let body = fixture().encode_to_vec();
        assert!(apply(&body, &mut state, &mut sync, 1, 1024).is_err());
        assert_eq!(state, InfoSyncPushState::default());
        assert_eq!(sync, OnlineSyncState::default());
        assert!(apply(&body, &mut state, &mut sync, 8, 3).is_err());
        assert_eq!(state, InfoSyncPushState::default());
        assert_eq!(sync, OnlineSyncState::default());
    }

    #[test]
    fn malformed_and_oversized_state_fail_closed() {
        let mut state = InfoSyncPushState::default();
        let mut sync = OnlineSyncState::default();
        assert!(apply(&[], &mut state, &mut sync, 8, 1024).is_err());
        let packet = Packet {
            error_message: "x".repeat(64 * 1024 + 1),
            ..Packet::default()
        };
        assert!(apply(&packet.encode_to_vec(), &mut state, &mut sync, 8, 1024,).is_err());
    }

    fn fixture() -> Packet {
        Packet {
            result: 0,
            push_flag: 1,
            push_sequence: 2,
            retry_flag: 3,
            use_initial_cache_data: 4,
            group_nodes: vec![GroupNode {
                group_code: 42,
                group_sequence: 8,
                read_message_sequence: 7,
                mask: 6,
                longest_message_time: 40,
                has_message: true,
                latest_message_time: 41,
                longest_message_sequence: 9,
                important_message_latest_sequence: 10,
                group_max_event_sequence: 11,
                need_check_sequence_on_aio_open: 12,
                ..GroupNode::default()
            }],
            group_notifications: Some(GroupNotifications {
                items: vec![GroupNotification {
                    group_code: 42,
                    start_sequence: 13,
                    end_sequence: 14,
                    messages: vec![vec![1, 2]],
                    last_speak_timestamp: 44,
                }],
            }),
            system_notifications: Some(SystemNotifications {
                items: vec![SystemNotification {
                    peer_uin: 88,
                    peer_uid: "peer".to_owned(),
                    last_speak_timestamp: 55,
                    messages: vec![vec![3, 4]],
                }],
            }),
            auxiliary_state: Some(vec![5, 6, 7]),
            guild_node: Some(GuildNode { peer_id: 77 }),
            recent_activity: Some(RecentActivity {
                primary_peers: vec![PeerActivity {
                    peer_uid: "peer".to_owned(),
                    timestamp: 55,
                }],
                secondary_peers: Vec::new(),
                groups: vec![GroupActivity {
                    group_uin: 42,
                    timestamp: 44,
                }],
            }),
            roam_message_optimize_flag: 5,
            group_guild_flag: 6,
            ..Packet::default()
        }
    }
}
