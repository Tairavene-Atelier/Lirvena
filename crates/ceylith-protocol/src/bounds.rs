/// Absolute maximum accepted outer frame length.
pub const HARD_MAX_OUTER_FRAME_LEN: usize = 2 * 1024 * 1024;
/// Default maximum encoded inner frame length.
pub const DEFAULT_INNER_FRAME_LEN: usize = 60 * 1024;
/// Default maximum ciphertext including the fixed 16-byte Noise tag.
pub const DEFAULT_SECURE_CIPHERTEXT_LEN: usize = DEFAULT_INNER_FRAME_LEN + 16;
/// Maximum encrypted access token length.
pub const MAX_ACCESS_TOKEN_LEN: usize = 512;
/// Maximum runtime lease representation length.
pub const MAX_RUNTIME_LEASE_LEN: usize = 512;
/// Maximum signed public profile manifest length.
pub const MAX_MANIFEST_LEN: usize = 32 * 1024;
/// Maximum value length for one opaque slot.
pub const MAX_OPAQUE_SLOT_LEN: usize = 4 * 1024;
/// Maximum number of opaque slots in one exchange.
pub const MAX_OPAQUE_SLOTS: usize = 64;
/// Maximum aggregate opaque slot value length in one exchange.
pub const MAX_OPAQUE_AGGREGATE_LEN: usize = 48 * 1024;
/// Maximum opaque payload carried by one Watch event.
pub const MAX_WATCH_PAYLOAD_LEN: usize = 16 * 1024;
/// Maximum contract identifiers advertised in one list.
pub(crate) const MAX_CONTRACTS: usize = 128;
/// Maximum long-poll duration accepted by the public contract.
pub const MAX_WATCH_WAIT_MS: u32 = 30_000;
/// Maximum opaque response body returned by one current-transport action.
pub const MAX_ACTION_PAYLOAD_LEN: usize = 48 * 1024;
/// Maximum opaque action body accepted by the compiled executor.
pub const MAX_ACTION_BODY_LEN: usize = 48 * 1024;
/// Maximum opaque local route selector length.
pub const MAX_ACTION_ROUTE_LEN: usize = 512;
/// Maximum numeric marks carried by one action.
pub const MAX_ACTION_MARKS: usize = 16;
/// Maximum bytes in one numeric action mark.
pub const MAX_ACTION_MARK_LEN: usize = 4 * 1024;
/// Maximum aggregate bytes across one action's numeric marks.
pub const MAX_ACTION_MARK_AGGREGATE_LEN: usize = 8 * 1024;
/// Maximum action timeout accepted by the compiled executor.
pub const MAX_ACTION_TIMEOUT_MS: u32 = 30_000;
/// Maximum server-directed delay before an action may be written.
pub const MAX_ACTION_DELAY_MS: u32 = 300_000;
