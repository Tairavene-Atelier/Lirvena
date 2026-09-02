//! Profile-driven QQ online packet codecs for Lirvena.

mod error;
mod heartbeat;
mod info_sync;
mod model;
mod proto;
mod push;
mod register;

pub use error::OnlinePacketError;
pub use heartbeat::{HeartbeatOutcome, encode_heartbeat, parse_heartbeat_response};
pub use info_sync::{InfoSyncOutcome, encode_info_sync, parse_info_sync_response};
pub use model::{HeartbeatInput, InfoSyncInput, OnlineDevice, OnlineSyncState, RegisterInput};
pub use push::{
    InfoSyncPushOutcome, InfoSyncPushState, InfoSyncPushSummary, ProtectiveNotice, PushOutcome,
    PushProcessor,
};
pub use register::{RegisterOutcome, encode_register, parse_register_response};
