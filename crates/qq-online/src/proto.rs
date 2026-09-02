use std::collections::HashMap;

use prost::Message;

#[derive(Clone, PartialEq, Message)]
pub(super) struct DeviceInfo {
    #[prost(string, tag = "1")]
    pub user: String,
    #[prost(string, tag = "2")]
    pub os: String,
    #[prost(string, tag = "3")]
    pub os_version: String,
    #[prost(string, optional, tag = "4")]
    pub vendor_name: Option<String>,
    #[prost(string, tag = "5")]
    pub os_lower: String,
}

#[derive(Clone, Copy, PartialEq, Message)]
pub(super) struct BusinessInfo {
    #[prost(uint32, tag = "1")]
    pub notify_switch: u32,
    #[prost(uint32, tag = "2")]
    pub bind_uin_notify_switch: u32,
}

#[derive(Clone, Copy, PartialEq, Message)]
pub(super) struct SilentControl {
    #[prost(int32, tag = "1")]
    pub local_silence: i32,
    #[prost(int32, tag = "2")]
    pub silence_version: i32,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct RegisterInfo {
    #[prost(string, optional, tag = "1")]
    pub guid: Option<String>,
    #[prost(int32, optional, tag = "2")]
    pub kick_pc: Option<i32>,
    #[prost(string, optional, tag = "3")]
    pub current_version: Option<String>,
    #[prost(int32, optional, tag = "4")]
    pub first_register: Option<i32>,
    #[prost(int32, optional, tag = "5")]
    pub locale_id: Option<i32>,
    #[prost(message, optional, tag = "6")]
    pub device: Option<DeviceInfo>,
    #[prost(int32, optional, tag = "7")]
    pub set_mute: Option<i32>,
    #[prost(int32, optional, tag = "8")]
    pub vendor_type: Option<i32>,
    #[prost(int32, optional, tag = "9")]
    pub register_type: Option<i32>,
    #[prost(message, optional, tag = "10")]
    pub business: Option<BusinessInfo>,
    #[prost(int32, optional, tag = "11")]
    pub battery_state: Option<i32>,
    #[prost(uint32, optional, tag = "12")]
    pub field_12: Option<u32>,
    #[prost(message, optional, tag = "14")]
    pub silence: Option<SilentControl>,
    #[prost(uint32, optional, tag = "16")]
    pub scene: Option<u32>,
    #[prost(uint32, optional, tag = "17")]
    pub background_seconds: Option<u32>,
    #[prost(bool, optional, tag = "18")]
    pub chat_on_focus: Option<bool>,
}

#[derive(Clone, Copy, PartialEq, Message)]
pub(super) struct DirectCookie {
    #[prost(uint64, tag = "1")]
    pub last_message_time: u64,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct DirectSync {
    #[prost(message, optional, tag = "1")]
    pub current_cookie: Option<DirectCookie>,
    #[prost(uint64, tag = "2")]
    pub last_message_time: u64,
    #[prost(message, optional, tag = "3")]
    pub previous_cookie: Option<DirectCookie>,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct NormalConfig {
    #[prost(map = "uint32, int32", tag = "1")]
    pub integer_values: HashMap<u32, i32>,
}

#[derive(Clone, Copy, PartialEq, Message)]
pub(super) struct AuxiliaryState {
    #[prost(uint32, optional, tag = "1")]
    pub group_code: Option<u32>,
    #[prost(uint32, tag = "2")]
    pub flag: u32,
}

#[derive(Clone, Copy, PartialEq, Message)]
pub(super) struct ApplicationState {
    #[prost(uint32, optional, tag = "1")]
    pub delayed: Option<u32>,
    #[prost(uint32, optional, tag = "2")]
    pub application_status: Option<u32>,
    #[prost(uint32, optional, tag = "3")]
    pub silence_status: Option<u32>,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct InfoSyncRequest {
    #[prost(uint32, tag = "1")]
    pub sync_flag: u32,
    #[prost(uint32, optional, tag = "2")]
    pub request_random: Option<u32>,
    #[prost(uint32, optional, tag = "4")]
    pub active_status: Option<u32>,
    #[prost(uint64, optional, tag = "5")]
    pub group_last_message_time: Option<u64>,
    #[prost(message, optional, tag = "6")]
    pub direct_sync: Option<DirectSync>,
    #[prost(message, optional, tag = "8")]
    pub normal_config: Option<NormalConfig>,
    #[prost(message, optional, tag = "9")]
    pub register: Option<RegisterInfo>,
    #[prost(message, optional, tag = "10")]
    pub auxiliary: Option<AuxiliaryState>,
    #[prost(message, optional, tag = "11")]
    pub application: Option<ApplicationState>,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct RegisterResponse {
    #[prost(int32, tag = "1")]
    pub result: i32,
    #[prost(string, optional, tag = "2")]
    pub message: Option<String>,
    #[prost(message, optional, tag = "9")]
    pub silence: Option<SilentControl>,
}

#[derive(Clone, Copy, PartialEq, Message)]
pub(super) struct DelayedSync {
    #[prost(bool, tag = "1")]
    pub enabled: bool,
    #[prost(uint32, tag = "2")]
    pub delay_seconds: u32,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct InfoSyncResponse {
    #[prost(int32, tag = "1")]
    pub result: i32,
    #[prost(string, tag = "2")]
    pub message: String,
    #[prost(uint32, tag = "3")]
    pub response_random: u32,
    #[prost(message, optional, tag = "7")]
    pub register: Option<RegisterResponse>,
    #[prost(message, optional, tag = "10")]
    pub delayed_sync: Option<DelayedSync>,
}

#[derive(Clone, Copy, PartialEq, Message)]
pub(super) struct HeartbeatRequest {
    #[prost(int32, tag = "1")]
    pub heartbeat_type: i32,
    #[prost(message, optional, tag = "2")]
    pub silence: Option<HeartbeatSilence>,
    #[prost(uint32, tag = "3")]
    pub battery_state: u32,
    #[prost(uint64, tag = "4")]
    pub unix_seconds: u64,
}

#[derive(Clone, Copy, PartialEq, Message)]
pub(super) struct HeartbeatSilence {
    #[prost(uint32, tag = "1")]
    pub local_silence: u32,
    #[prost(uint32, tag = "2")]
    pub silence_version: u32,
}

#[derive(Clone, Copy, PartialEq, Message)]
pub(super) struct HeartbeatResponse {
    #[prost(int32, tag = "3")]
    pub next_interval_seconds: i32,
}
