use core::fmt;

const MAX_TEXT_BYTES: usize = 96;

/// User-managed synthetic power description without protocol bit encodings.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DevicePower {
    /// Desktop profile without a battery.
    Desktop,
    /// Portable profile with a bounded synthetic battery state.
    Portable {
        /// Synthetic remaining battery percentage.
        percent: u8,
        /// Whether the synthetic device is charging.
        charging: bool,
    },
}

/// User-managed synthetic device profile.
///
/// This type deliberately contains no application version, security-chain value, profile switch,
/// or other Ceylith-managed material.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeviceProfile {
    guid: [u8; 16],
    mac_address: [u8; 6],
    name: String,
    model: String,
    system_kernel: String,
    kernel_version: String,
    power: DevicePower,
}

impl DeviceProfile {
    /// Validates a user-managed synthetic device profile.
    ///
    /// # Errors
    ///
    /// Returns an error for empty identifiers, unsafe text, invalid MAC addresses, or impossible
    /// battery percentages.
    pub fn new(
        guid: [u8; 16],
        mac_address: [u8; 6],
        name: String,
        model: String,
        system_kernel: String,
        kernel_version: String,
        power: DevicePower,
    ) -> Result<Self, DeviceProfileError> {
        let text = [&name, &model, &system_kernel, &kernel_version];
        let invalid_power = matches!(power, DevicePower::Portable { percent, .. } if percent > 100);
        if guid == [0; 16]
            || mac_address == [0; 6]
            || mac_address == [0xff; 6]
            || mac_address[0] & 1 != 0
            || text.into_iter().any(|value| !valid_text(value))
            || invalid_power
        {
            return Err(DeviceProfileError);
        }
        Ok(Self {
            guid,
            mac_address,
            name,
            model,
            system_kernel,
            kernel_version,
            power,
        })
    }

    #[must_use]
    /// Returns the stable user-managed GUID.
    pub const fn guid(&self) -> &[u8; 16] {
        &self.guid
    }

    #[must_use]
    /// Returns the locally administered unicast MAC address.
    pub const fn mac_address(&self) -> &[u8; 6] {
        &self.mac_address
    }

    #[must_use]
    /// Returns the user-managed device name.
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    /// Returns the user-managed hardware model description.
    pub fn model(&self) -> &str {
        &self.model
    }

    #[must_use]
    /// Returns the synthetic operating-system kernel family.
    pub fn system_kernel(&self) -> &str {
        &self.system_kernel
    }

    #[must_use]
    /// Returns the synthetic operating-system kernel release.
    pub fn kernel_version(&self) -> &str {
        &self.kernel_version
    }

    #[must_use]
    /// Returns the structured synthetic power description.
    pub const fn power(&self) -> DevicePower {
        self.power
    }
}

/// Rejected user-managed device profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeviceProfileError;

impl fmt::Display for DeviceProfileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("synthetic device profile rejected")
    }
}

impl std::error::Error for DeviceProfileError {}

fn valid_text(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_TEXT_BYTES
        && value.trim() == value
        && !value.chars().any(char::is_control)
}

#[cfg(test)]
mod tests {
    use super::{DevicePower, DeviceProfile};

    #[test]
    fn accepts_external_profile_without_fixed_material() {
        let profile = DeviceProfile::new(
            [1; 16],
            [2, 0, 0, 0, 0, 1],
            "Lirvena device".to_owned(),
            "Synthetic desktop".to_owned(),
            "Linux".to_owned(),
            "6.8.0-generic".to_owned(),
            DevicePower::Desktop,
        );
        assert!(profile.is_ok());
    }

    #[test]
    fn rejects_multicast_mac_and_invalid_power() {
        assert!(
            DeviceProfile::new(
                [1; 16],
                [3, 0, 0, 0, 0, 1],
                "Lirvena device".to_owned(),
                "Synthetic portable".to_owned(),
                "Linux".to_owned(),
                "6.8.0-generic".to_owned(),
                DevicePower::Portable {
                    percent: 101,
                    charging: true,
                },
            )
            .is_err()
        );
    }
}
