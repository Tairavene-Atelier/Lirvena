/// Bounded public fields retained from a protective-offline Push.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProtectiveNotice {
    /// Public title text, truncated by protobuf and Profile bounds.
    pub title: String,
    /// Public user-facing detail text.
    pub detail: String,
    /// Account identifier carried by the remote notice, if any.
    pub account: u32,
    /// Remote reason code.
    pub reason_code: i32,
    /// Remote control code.
    pub control_code: i32,
    /// Remote session code.
    pub session_code: i32,
}

/// Result of one compiled Push primitive.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PushOutcome {
    /// Send a canonical response on the selected route.
    Ack {
        /// Profile-selected response route.
        route: String,
        /// Compiled bounded response body.
        body: Vec<u8>,
    },
    /// Accept only bounded diagnostic metadata.
    Observed,
    /// Discard the current QQ transport generation immediately.
    ProtectiveOffline(ProtectiveNotice),
    /// Deliver an authenticated body to the compiled message decoder.
    Message(Vec<u8>),
    /// Apply synchronization cursors and deliver its bounded embedded messages.
    InfoSync(InfoSyncPushOutcome),
}
use super::InfoSyncPushOutcome;
