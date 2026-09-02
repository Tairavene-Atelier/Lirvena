use ceylith_protocol::ProfileId;

use crate::{OpaqueSlots, ProfileValueError};

pub(super) const MAX_VERSION_LEN: usize = 32;
pub(super) const MAX_PACKAGE_LEN: usize = 96;
pub(super) const MAX_OS_LEN: usize = 32;
pub(super) const MAX_LOGIN_SDK_LEN: usize = 64;

/// Owned ordinary fields used to construct a Linux NTQQ profile.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LinuxNtProfileSpec {
    /// Signed Ceylith profile release identifier.
    pub profile_id: ProfileId,
    /// Human-readable upstream client version.
    pub client_version: String,
    /// Ordinary application identifier.
    pub app_id: u32,
    /// Ordinary sub-application identifier.
    pub sub_app_id: u32,
    /// Ordinary QR-login application identifier.
    pub qr_app_id: u32,
    /// Ordinary upstream build number.
    pub app_client_version: u16,
    /// Upstream package identifier.
    pub package_name: String,
    /// Upstream platform label.
    pub operating_system: String,
    /// Upstream ptlogin version.
    pub pt_version: String,
    /// SSO protocol generation.
    pub sso_version: u32,
    /// Ordinary capability bitmap.
    pub misc_bitmap: u32,
    /// Login SDK generation used by the selected upstream build.
    pub login_sdk: String,
    /// Main login signature capability map.
    pub main_sig_map: u32,
    /// Secondary login signature capability map.
    pub sub_sig_map: u32,
    /// Login request capability bitmap.
    pub login_misc_bitmap: u32,
    /// Minimum compiled Lirvena runtime ABI.
    pub runtime_abi: u32,
}

/// Validated Linux NTQQ profile consumed by login and transport layers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LinuxNtProfile {
    spec: LinuxNtProfileSpec,
    slots: OpaqueSlots,
}

impl LinuxNtProfile {
    /// Validates and creates a profile.
    ///
    /// # Errors
    ///
    /// Returns an error for zero numeric fields or invalid public text fields.
    pub fn new(spec: LinuxNtProfileSpec, slots: OpaqueSlots) -> Result<Self, ProfileValueError> {
        validate_spec(&spec)?;
        Ok(Self { spec, slots })
    }

    /// Returns the signed public profile identifier.
    #[must_use]
    pub const fn profile_id(&self) -> ProfileId {
        self.spec.profile_id
    }

    /// Returns the upstream client version.
    #[must_use]
    pub fn client_version(&self) -> &str {
        &self.spec.client_version
    }

    /// Returns the ordinary application identifier.
    #[must_use]
    pub const fn app_id(&self) -> u32 {
        self.spec.app_id
    }

    /// Returns the ordinary sub-application identifier.
    #[must_use]
    pub const fn sub_app_id(&self) -> u32 {
        self.spec.sub_app_id
    }

    /// Returns the ordinary QR-login application identifier.
    #[must_use]
    pub const fn qr_app_id(&self) -> u32 {
        self.spec.qr_app_id
    }

    /// Returns the ordinary upstream build number.
    #[must_use]
    pub const fn app_client_version(&self) -> u16 {
        self.spec.app_client_version
    }

    /// Returns the package identifier.
    #[must_use]
    pub fn package_name(&self) -> &str {
        &self.spec.package_name
    }

    /// Returns the upstream platform label.
    #[must_use]
    pub fn operating_system(&self) -> &str {
        &self.spec.operating_system
    }

    /// Returns the upstream ptlogin version.
    #[must_use]
    pub fn pt_version(&self) -> &str {
        &self.spec.pt_version
    }

    /// Returns the SSO protocol generation.
    #[must_use]
    pub const fn sso_version(&self) -> u32 {
        self.spec.sso_version
    }

    /// Returns the ordinary capability bitmap.
    #[must_use]
    pub const fn misc_bitmap(&self) -> u32 {
        self.spec.misc_bitmap
    }

    /// Returns the login SDK generation.
    #[must_use]
    pub fn login_sdk(&self) -> &str {
        &self.spec.login_sdk
    }

    /// Returns the main login signature capability map.
    #[must_use]
    pub const fn main_sig_map(&self) -> u32 {
        self.spec.main_sig_map
    }

    /// Returns the secondary login signature capability map.
    #[must_use]
    pub const fn sub_sig_map(&self) -> u32 {
        self.spec.sub_sig_map
    }

    /// Returns the login request capability bitmap.
    #[must_use]
    pub const fn login_misc_bitmap(&self) -> u32 {
        self.spec.login_misc_bitmap
    }

    /// Returns the minimum compiled runtime ABI.
    #[must_use]
    pub const fn runtime_abi(&self) -> u32 {
        self.spec.runtime_abi
    }

    /// Returns the bounded numeric opaque slot collection.
    #[must_use]
    pub const fn opaque_slots(&self) -> &OpaqueSlots {
        &self.slots
    }
}

fn validate_spec(spec: &LinuxNtProfileSpec) -> Result<(), ProfileValueError> {
    if [
        spec.app_id,
        spec.sub_app_id,
        spec.qr_app_id,
        u32::from(spec.app_client_version),
        spec.sso_version,
        spec.main_sig_map,
        spec.login_misc_bitmap,
        spec.runtime_abi,
    ]
    .contains(&0)
    {
        return Err(ProfileValueError::ZeroNumber);
    }
    validate_text(&spec.client_version, MAX_VERSION_LEN, version_character)?;
    validate_text(&spec.package_name, MAX_PACKAGE_LEN, package_character)?;
    validate_text(&spec.operating_system, MAX_OS_LEN, package_character)?;
    validate_text(&spec.pt_version, MAX_VERSION_LEN, version_character)?;
    validate_text(&spec.login_sdk, MAX_LOGIN_SDK_LEN, package_character)
}

fn validate_text(
    value: &str,
    maximum: usize,
    allowed: impl Fn(u8) -> bool,
) -> Result<(), ProfileValueError> {
    if value.is_empty() || value.len() > maximum || !value.bytes().all(allowed) {
        Err(ProfileValueError::InvalidText)
    } else {
        Ok(())
    }
}

const fn version_character(value: u8) -> bool {
    value.is_ascii_digit() || value == b'.' || value == b'-'
}

const fn package_character(value: u8) -> bool {
    value.is_ascii_alphanumeric() || value == b'.' || value == b'_'
}
