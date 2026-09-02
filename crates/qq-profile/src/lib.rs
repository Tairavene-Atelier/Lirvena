//! QQ profile negotiation boundary for Lirvena.

mod error;
mod linux;
mod manifest;
mod online;
mod online_packet;
mod push;

pub use ceylith_protocol::{OpaqueSlot, OpaqueSlotId, OpaqueSlots};
pub use error::ProfileValueError;
pub use linux::{LinuxNtProfile, LinuxNtProfileSpec};
pub use manifest::{ProfileManifestError, decode_linux_manifest, encode_linux_manifest};
pub use online::{
    ONLINE_PLAN_SLOT_ID, OnlinePlanManifestError, decode_online_plan, encode_online_plan,
};
pub use online_packet::{
    ONLINE_PACKET_PLAN_SLOT_ID, OnlinePacketPlan, OnlinePacketPlanError, OnlinePacketPlanSpec,
    OnlinePacketTuning, OnlinePacketTuningSpec, decode_online_packet_plan,
    encode_online_packet_plan,
};
pub use push::{
    PUSH_PLAN_SLOT_ID, PushBehavior, PushPlan, PushPlanEntry, PushPlanError, PushPlanSpec,
    decode_push_plan, encode_push_plan,
};
