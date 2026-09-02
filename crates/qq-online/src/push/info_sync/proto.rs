use prost::Message;

#[derive(Clone, PartialEq, Message)]
pub(super) struct Packet {
    #[prost(uint32, tag = "1")]
    pub result: u32,
    #[prost(string, tag = "2")]
    pub error_message: String,
    #[prost(uint32, tag = "3")]
    pub push_flag: u32,
    #[prost(uint32, tag = "4")]
    pub push_sequence: u32,
    #[prost(uint32, tag = "5")]
    pub retry_flag: u32,
    #[prost(message, repeated, tag = "6")]
    pub group_nodes: Vec<GroupNode>,
    #[prost(message, optional, tag = "7")]
    pub group_notifications: Option<GroupNotifications>,
    #[prost(message, optional, tag = "8")]
    pub system_notifications: Option<SystemNotifications>,
    #[prost(bytes = "vec", optional, tag = "9")]
    pub auxiliary_state: Option<Vec<u8>>,
    #[prost(uint32, tag = "10")]
    pub use_initial_cache_data: u32,
    #[prost(message, optional, tag = "11")]
    pub guild_node: Option<GuildNode>,
    #[prost(message, optional, tag = "12")]
    pub recent_activity: Option<RecentActivity>,
    #[prost(uint32, tag = "13")]
    pub roam_message_optimize_flag: u32,
    #[prost(uint32, tag = "14")]
    pub group_guild_flag: u32,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct GroupNode {
    #[prost(uint64, tag = "1")]
    pub group_code: u64,
    #[prost(uint64, tag = "2")]
    pub group_sequence: u64,
    #[prost(uint64, tag = "3")]
    pub read_message_sequence: u64,
    #[prost(uint64, tag = "4")]
    pub mask: u64,
    #[prost(uint64, tag = "5")]
    pub longest_message_time: u64,
    #[prost(bool, tag = "6")]
    pub has_message: bool,
    #[prost(uint64, tag = "8")]
    pub latest_message_time: u64,
    #[prost(string, tag = "9")]
    pub peer_name: String,
    #[prost(uint64, tag = "10")]
    pub longest_message_sequence: u64,
    #[prost(uint64, tag = "11")]
    pub uin_flag_ex2: u64,
    #[prost(uint32, tag = "12")]
    pub important_message_latest_sequence: u32,
    #[prost(uint32, tag = "13")]
    pub group_max_event_sequence: u32,
    #[prost(uint32, tag = "14")]
    pub random: u32,
    #[prost(uint32, tag = "15")]
    pub need_check_sequence_on_aio_open: u32,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct GroupNotifications {
    #[prost(message, repeated, tag = "3")]
    pub items: Vec<GroupNotification>,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct GroupNotification {
    #[prost(uint64, tag = "3")]
    pub group_code: u64,
    #[prost(uint32, tag = "4")]
    pub start_sequence: u32,
    #[prost(uint32, tag = "5")]
    pub end_sequence: u32,
    #[prost(bytes = "vec", repeated, tag = "6")]
    pub messages: Vec<Vec<u8>>,
    #[prost(uint64, tag = "8")]
    pub last_speak_timestamp: u64,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct SystemNotifications {
    #[prost(message, repeated, tag = "4")]
    pub items: Vec<SystemNotification>,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct SystemNotification {
    #[prost(uint64, tag = "1")]
    pub peer_uin: u64,
    #[prost(string, tag = "2")]
    pub peer_uid: String,
    #[prost(uint64, tag = "5")]
    pub last_speak_timestamp: u64,
    #[prost(bytes = "vec", repeated, tag = "8")]
    pub messages: Vec<Vec<u8>>,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct GuildNode {
    #[prost(uint64, tag = "1")]
    pub peer_id: u64,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct RecentActivity {
    #[prost(message, repeated, tag = "1")]
    pub primary_peers: Vec<PeerActivity>,
    #[prost(message, repeated, tag = "2")]
    pub secondary_peers: Vec<PeerActivity>,
    #[prost(message, repeated, tag = "3")]
    pub groups: Vec<GroupActivity>,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct PeerActivity {
    #[prost(string, tag = "1")]
    pub peer_uid: String,
    #[prost(uint64, tag = "2")]
    pub timestamp: u64,
}

#[derive(Clone, Copy, PartialEq, Message)]
pub(super) struct GroupActivity {
    #[prost(uint64, tag = "1")]
    pub group_uin: u64,
    #[prost(uint64, tag = "2")]
    pub timestamp: u64,
}
