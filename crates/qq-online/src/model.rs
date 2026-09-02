use crate::OnlinePacketError;
use qq_profile::OnlinePacketTuning;

const GUID_HEX_LEN: usize = 32;
const MAX_DEVICE_TEXT_LEN: usize = 128;
const MAX_VERSION_LEN: usize = 64;

/// Validated ordinary device facts used by online registration packets.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OnlineDevice {
    guid_hex: String,
    name: String,
    operating_system: String,
    operating_system_version: String,
    vendor_operating_system: String,
    client_version: String,
    battery_state: u32,
}

impl OnlineDevice {
    /// Creates a bounded online device snapshot.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed GUID text, oversized text or battery values above 100.
    pub fn new(
        guid_hex: &str,
        name: String,
        operating_system: String,
        operating_system_version: String,
        vendor_operating_system: String,
        client_version: String,
        battery_state: u32,
    ) -> Result<Self, OnlinePacketError> {
        if guid_hex.len() != GUID_HEX_LEN
            || !guid_hex.bytes().all(|byte| byte.is_ascii_hexdigit())
            || !valid_text(&name, MAX_DEVICE_TEXT_LEN, false)
            || !valid_text(&operating_system, MAX_DEVICE_TEXT_LEN, false)
            || !valid_text(&operating_system_version, MAX_DEVICE_TEXT_LEN, true)
            || !valid_text(&vendor_operating_system, MAX_DEVICE_TEXT_LEN, false)
            || !valid_text(&client_version, MAX_VERSION_LEN, false)
            || battery_state > 100
        {
            return Err(OnlinePacketError);
        }
        Ok(Self {
            guid_hex: guid_hex.to_ascii_uppercase(),
            name,
            operating_system,
            operating_system_version,
            vendor_operating_system,
            client_version,
            battery_state,
        })
    }

    pub(super) fn guid_hex(&self) -> &str {
        &self.guid_hex
    }

    pub(super) fn name(&self) -> &str {
        &self.name
    }

    pub(super) fn operating_system(&self) -> &str {
        &self.operating_system
    }

    pub(super) fn operating_system_version(&self) -> &str {
        &self.operating_system_version
    }

    pub(super) fn vendor_operating_system(&self) -> &str {
        &self.vendor_operating_system
    }

    pub(super) fn client_version(&self) -> &str {
        &self.client_version
    }

    /// Returns the bounded battery percentage represented by this snapshot.
    #[must_use]
    pub const fn battery_state(&self) -> u32 {
        self.battery_state
    }
}

/// Mutable ordinary cursor and presence state for one online generation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OnlineSyncState {
    /// Whether this is the first proxy-online registration in the generation.
    pub first_register: bool,
    /// Current active-status value.
    pub active_status: u32,
    /// Latest group message timestamp.
    pub group_last_message_time: u64,
    /// Latest direct-message timestamp.
    pub direct_last_message_time: u64,
    /// Previous direct-message timestamp.
    pub previous_direct_message_time: u64,
    /// Last group code observed by synchronization.
    pub last_group_code: u32,
    /// Current application status.
    pub application_status: u32,
    /// Current local silence setting.
    pub local_silence: u32,
    /// Current silence-setting version.
    pub silence_version: u32,
    /// Current client scene.
    pub scene: u32,
    /// Seconds spent in background.
    pub background_seconds: u32,
    /// Whether a chat currently owns focus.
    pub chat_on_focus: bool,
}

impl Default for OnlineSyncState {
    fn default() -> Self {
        Self {
            first_register: true,
            active_status: 0,
            group_last_message_time: 0,
            direct_last_message_time: 0,
            previous_direct_message_time: 0,
            last_group_code: 0,
            application_status: 0,
            local_silence: 0,
            silence_version: 0,
            scene: 0,
            background_seconds: 0,
            chat_on_focus: false,
        }
    }
}

/// Borrowed input for an initial or delayed synchronization request.
#[derive(Clone, Copy, Debug)]
pub struct InfoSyncInput<'a> {
    /// Validated device snapshot.
    pub device: &'a OnlineDevice,
    /// Current generation cursor state.
    pub state: OnlineSyncState,
    /// Version-selected numeric values.
    pub tuning: OnlinePacketTuning,
    /// Non-zero request correlation value.
    pub request_random: u32,
    /// Whether QQ requested this delayed continuation.
    pub delayed: bool,
}

/// Borrowed input for the optional standalone registration.
#[derive(Clone, Copy, Debug)]
pub struct RegisterInput<'a> {
    /// Validated device snapshot.
    pub device: &'a OnlineDevice,
    /// Current generation cursor state.
    pub state: OnlineSyncState,
    /// Version-selected numeric values.
    pub tuning: OnlinePacketTuning,
}

/// Input for one modern online heartbeat.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HeartbeatInput {
    /// Current generation cursor state.
    pub state: OnlineSyncState,
    /// Version-selected numeric values.
    pub tuning: OnlinePacketTuning,
    /// Current UTC Unix time in seconds.
    pub unix_seconds: u64,
    /// Current bounded battery percentage.
    pub battery_state: u32,
}

fn valid_text(value: &str, maximum: usize, allow_empty: bool) -> bool {
    (allow_empty || !value.is_empty())
        && value.len() <= maximum
        && !value.chars().any(char::is_control)
}
