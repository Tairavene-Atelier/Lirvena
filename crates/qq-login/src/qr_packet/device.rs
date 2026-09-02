use qq_domain::DeviceProfile;

/// Client-managed synthetic device profile used by the login runtime.
///
/// Packet builders may only emit fields backed by frozen evidence. Keeping the full profile here
/// does not imply that every external description is sent to QQ.
#[derive(Clone, Eq, PartialEq)]
pub struct QrDevice {
    profile: DeviceProfile,
}

impl QrDevice {
    /// Creates a QR device from an already validated synthetic profile.
    #[must_use]
    pub const fn new(profile: DeviceProfile) -> Self {
        Self { profile }
    }

    /// Returns the exact 16-byte device GUID representation.
    #[must_use]
    pub const fn guid(&self) -> &[u8; 16] {
        self.profile.guid()
    }

    /// Returns the user-facing device name.
    #[must_use]
    pub fn name(&self) -> &str {
        self.profile.name()
    }

    /// Returns the complete validated external device profile.
    #[must_use]
    pub const fn profile(&self) -> &DeviceProfile {
        &self.profile
    }
}

impl core::fmt::Debug for QrDevice {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("QrDevice")
            .field("guid", &"<redacted>")
            .field("name_len", &self.profile.name().len())
            .field("mac_address", &"<redacted>")
            .field("model_len", &self.profile.model().len())
            .finish()
    }
}
