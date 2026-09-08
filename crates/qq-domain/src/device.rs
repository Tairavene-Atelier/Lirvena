use core::fmt;

const MAX_TEXT_BYTES: usize = 96;
const SNAPSHOT_VERSION: u8 = 1;

/// User-visible synthetic Linux portrait shared by every QQ-facing consumer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DevicePortrait {
    archetype: String,
    profile_category: String,
    hostname: String,
    device_name: String,
    hardware_model: String,
    os_release: String,
    distro: String,
    desktop_environment: String,
    session_type: String,
    login_name: String,
    network_interface: String,
    root_filesystem_bytes: String,
    memory_total_kib: String,
    timezone: String,
}

impl DevicePortrait {
    /// Constructs one coherent user-managed synthetic Linux portrait.
    ///
    /// # Errors
    ///
    /// Returns an error when a text value is empty, unsafe, or outside its bound, or when a
    /// numeric text field is not a positive decimal integer.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        archetype: String,
        profile_category: String,
        hostname: String,
        device_name: String,
        hardware_model: String,
        os_release: String,
        distro: String,
        desktop_environment: String,
        session_type: String,
        login_name: String,
        network_interface: String,
        root_filesystem_bytes: String,
        memory_total_kib: String,
        timezone: String,
    ) -> Result<Self, DeviceProfileError> {
        let text = [
            &archetype,
            &profile_category,
            &hostname,
            &device_name,
            &hardware_model,
            &os_release,
            &distro,
            &desktop_environment,
            &session_type,
            &login_name,
            &network_interface,
            &root_filesystem_bytes,
            &memory_total_kib,
            &timezone,
        ];
        if text.into_iter().any(|value| !valid_text(value))
            || !valid_hostname(&hostname)
            || !valid_positive_decimal(&root_filesystem_bytes)
            || !valid_positive_decimal(&memory_total_kib)
        {
            return Err(DeviceProfileError);
        }
        Ok(Self {
            archetype,
            profile_category,
            hostname,
            device_name,
            hardware_model,
            os_release,
            distro,
            desktop_environment,
            session_type,
            login_name,
            network_interface,
            root_filesystem_bytes,
            memory_total_kib,
            timezone,
        })
    }

    /// Returns the stable template identifier.
    #[must_use]
    pub fn archetype(&self) -> &str {
        &self.archetype
    }
    /// Returns the template category.
    #[must_use]
    pub fn profile_category(&self) -> &str {
        &self.profile_category
    }
    /// Returns the synthetic host name used by QQ-facing device-name fields.
    #[must_use]
    pub fn hostname(&self) -> &str {
        &self.hostname
    }
    /// Returns the short hardware display name.
    #[must_use]
    pub fn device_name(&self) -> &str {
        &self.device_name
    }
    /// Returns the vendor-qualified hardware model.
    #[must_use]
    pub fn hardware_model(&self) -> &str {
        &self.hardware_model
    }
    /// Returns the complete synthetic operating-system release.
    #[must_use]
    pub fn os_release(&self) -> &str {
        &self.os_release
    }
    /// Returns the synthetic distribution name.
    #[must_use]
    pub fn distro(&self) -> &str {
        &self.distro
    }
    /// Returns the desktop environment.
    #[must_use]
    pub fn desktop_environment(&self) -> &str {
        &self.desktop_environment
    }
    /// Returns the graphical session type.
    #[must_use]
    pub fn session_type(&self) -> &str {
        &self.session_type
    }
    /// Returns the synthetic login name.
    #[must_use]
    pub fn login_name(&self) -> &str {
        &self.login_name
    }
    /// Returns the synthetic primary network interface.
    #[must_use]
    pub fn network_interface(&self) -> &str {
        &self.network_interface
    }
    /// Returns the synthetic root-filesystem size in bytes.
    #[must_use]
    pub fn root_filesystem_bytes(&self) -> &str {
        &self.root_filesystem_bytes
    }
    /// Returns the synthetic total memory in KiB.
    #[must_use]
    pub fn memory_total_kib(&self) -> &str {
        &self.memory_total_kib
    }
    /// Returns the synthetic timezone.
    #[must_use]
    pub fn timezone(&self) -> &str {
        &self.timezone
    }
}

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
    device_name: String,
    model: String,
    system_kernel: String,
    kernel_version: String,
    power: DevicePower,
    portrait: DevicePortrait,
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
        device_name: String,
        model: String,
        system_kernel: String,
        kernel_version: String,
        power: DevicePower,
    ) -> Result<Self, DeviceProfileError> {
        let text = [&device_name, &model, &system_kernel, &kernel_version];
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
        let portrait = DevicePortrait::new(
            "custom".to_owned(),
            "custom".to_owned(),
            device_name.clone(),
            model.clone(),
            model.clone(),
            format!("{system_kernel} x86_64"),
            "Linux".to_owned(),
            "unknown".to_owned(),
            "x11".to_owned(),
            "user".to_owned(),
            "eth0".to_owned(),
            "499963174912".to_owned(),
            "16325684".to_owned(),
            "Asia/Shanghai".to_owned(),
        )?;
        Ok(Self {
            guid,
            mac_address,
            device_name,
            model,
            system_kernel,
            kernel_version,
            power,
            portrait,
        })
    }

    /// Constructs a profile from a complete external portrait.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid stable identifiers or an impossible battery percentage.
    pub fn from_portrait(
        guid: [u8; 16],
        mac_address: [u8; 6],
        portrait: DevicePortrait,
        power: DevicePower,
    ) -> Result<Self, DeviceProfileError> {
        let invalid_power = matches!(power, DevicePower::Portable { percent, .. } if percent > 100);
        if guid == [0; 16]
            || mac_address == [0; 6]
            || mac_address == [0xff; 6]
            || mac_address[0] & 1 != 0
            || invalid_power
        {
            return Err(DeviceProfileError);
        }
        let system_kernel = portrait
            .os_release()
            .strip_suffix(" x86_64")
            .unwrap_or(portrait.os_release())
            .to_owned();
        let kernel_version = system_kernel
            .strip_prefix("Linux ")
            .unwrap_or(&system_kernel)
            .to_owned();
        Ok(Self {
            guid,
            mac_address,
            device_name: portrait.hostname().to_owned(),
            model: portrait.hardware_model().to_owned(),
            system_kernel,
            kernel_version,
            power,
            portrait,
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
    /// Returns the user-managed name presented in QQ login and online packets.
    pub fn device_name(&self) -> &str {
        &self.device_name
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

    /// Returns the complete external synthetic portrait.
    #[must_use]
    pub const fn portrait(&self) -> &DevicePortrait {
        &self.portrait
    }

    /// Encodes the complete external portrait into a canonical bounded snapshot.
    ///
    /// The snapshot contains no application version or security-chain material.
    #[must_use]
    pub fn encode_snapshot(&self) -> Vec<u8> {
        let mut output = Vec::with_capacity(512);
        output.push(SNAPSHOT_VERSION);
        output.extend_from_slice(&self.guid);
        output.extend_from_slice(&self.mac_address);
        for value in [
            self.portrait.archetype(),
            self.portrait.profile_category(),
            self.portrait.hostname(),
            self.portrait.device_name(),
            self.portrait.hardware_model(),
            self.portrait.os_release(),
            self.portrait.distro(),
            self.portrait.desktop_environment(),
            self.portrait.session_type(),
            self.portrait.login_name(),
            self.portrait.network_interface(),
            self.portrait.root_filesystem_bytes(),
            self.portrait.memory_total_kib(),
            self.portrait.timezone(),
        ] {
            output.push(u8::try_from(value.len()).unwrap_or(u8::MAX));
            output.extend_from_slice(value.as_bytes());
        }
        match self.power {
            DevicePower::Desktop => output.extend_from_slice(&[0, 0, 0]),
            DevicePower::Portable { percent, charging } => {
                output.extend_from_slice(&[1, percent, u8::from(charging)]);
            }
        }
        output
    }

    /// Decodes a canonical external portrait snapshot.
    ///
    /// # Errors
    ///
    /// Returns an error for a malformed, non-canonical, or invalid snapshot.
    pub fn decode_snapshot(bytes: &[u8]) -> Result<Self, DeviceProfileError> {
        let mut reader = SnapshotReader::new(bytes);
        if reader.byte()? != SNAPSHOT_VERSION {
            return Err(DeviceProfileError);
        }
        let guid = reader.array()?;
        let mac_address = reader.array()?;
        let portrait = DevicePortrait::new(
            reader.text()?,
            reader.text()?,
            reader.text()?,
            reader.text()?,
            reader.text()?,
            reader.text()?,
            reader.text()?,
            reader.text()?,
            reader.text()?,
            reader.text()?,
            reader.text()?,
            reader.text()?,
            reader.text()?,
            reader.text()?,
        )?;
        let power = match (reader.byte()?, reader.byte()?, reader.byte()?) {
            (0, 0, 0) => DevicePower::Desktop,
            (1, percent, charging @ 0..=1) => DevicePower::Portable {
                percent,
                charging: charging == 1,
            },
            _ => return Err(DeviceProfileError),
        };
        if !reader.remaining().is_empty() {
            return Err(DeviceProfileError);
        }
        Self::from_portrait(guid, mac_address, portrait, power)
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

fn valid_hostname(value: &str) -> bool {
    value.len() <= 63
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        && !value.starts_with('-')
        && !value.ends_with('-')
}

fn valid_positive_decimal(value: &str) -> bool {
    value.bytes().all(|byte| byte.is_ascii_digit()) && value.parse::<u64>().is_ok_and(|n| n > 0)
}

struct SnapshotReader<'a>(&'a [u8]);

impl<'a> SnapshotReader<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self(bytes)
    }

    fn byte(&mut self) -> Result<u8, DeviceProfileError> {
        let (value, remaining) = self.0.split_first().ok_or(DeviceProfileError)?;
        self.0 = remaining;
        Ok(*value)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], DeviceProfileError> {
        let (value, remaining) = self.0.split_at_checked(N).ok_or(DeviceProfileError)?;
        self.0 = remaining;
        value.try_into().map_err(|_| DeviceProfileError)
    }

    fn text(&mut self) -> Result<String, DeviceProfileError> {
        let len = usize::from(self.byte()?);
        let (value, remaining) = self.0.split_at_checked(len).ok_or(DeviceProfileError)?;
        self.0 = remaining;
        core::str::from_utf8(value)
            .map(str::to_owned)
            .map_err(|_| DeviceProfileError)
    }

    const fn remaining(&self) -> &[u8] {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::{DevicePower, DeviceProfile};

    #[test]
    fn accepts_external_profile_without_fixed_material() {
        let profile = DeviceProfile::new(
            [1; 16],
            [2, 0, 0, 0, 0, 1],
            "uos-office-42".to_owned(),
            "Lenovo ThinkCentre M720q".to_owned(),
            "Linux 5.10.0-amd64-desktop".to_owned(),
            "5.10.0-amd64-desktop".to_owned(),
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
                "xps-dev-42".to_owned(),
                "Dell XPS 13 9310".to_owned(),
                "Linux 5.15.0-139-generic".to_owned(),
                "5.15.0-139-generic".to_owned(),
                DevicePower::Portable {
                    percent: 101,
                    charging: true,
                },
            )
            .is_err()
        );
    }

    #[test]
    fn external_snapshot_round_trips_canonically() -> Result<(), Box<dyn std::error::Error>> {
        let profile = DeviceProfile::new(
            [1; 16],
            [2, 0, 0, 0, 0, 1],
            "uos-office-42".to_owned(),
            "Lenovo ThinkCentre M720q".to_owned(),
            "Linux 5.10.0-amd64-desktop".to_owned(),
            "5.10.0-amd64-desktop".to_owned(),
            DevicePower::Desktop,
        )?;
        let encoded = profile.encode_snapshot();
        assert_eq!(DeviceProfile::decode_snapshot(&encoded)?, profile);
        let mut trailing = encoded;
        trailing.push(0);
        assert!(DeviceProfile::decode_snapshot(&trailing).is_err());
        Ok(())
    }
}
