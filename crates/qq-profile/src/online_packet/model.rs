use core::fmt;

const MAX_ROUTE_LEN: usize = 128;

/// Version-selected numeric online values supplied by a signed Profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OnlinePacketTuning {
    spec: OnlinePacketTuningSpec,
}

/// Numeric fields used to construct one validated online packet plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OnlinePacketTuningSpec {
    /// Initial synchronization flag mask.
    pub sync_flag: u32,
    /// Locale identifier.
    pub locale_id: i32,
    /// Vendor type for the embedded initial registration.
    pub initial_vendor_type: i32,
    /// Registration type for the embedded initial registration.
    pub initial_register_type: i32,
    /// Vendor type for the optional standalone registration.
    pub status_vendor_type: i32,
    /// Registration type for the optional standalone registration.
    pub status_register_type: i32,
    /// Fixed auxiliary flag carried by synchronization.
    pub auxiliary_flag: u32,
    /// Modern heartbeat type.
    pub heartbeat_type: i32,
}

impl OnlinePacketTuning {
    /// Validates version-selected numeric online fields.
    ///
    /// # Errors
    ///
    /// Returns an error for zero required values or negative enum-like values.
    pub fn new(spec: OnlinePacketTuningSpec) -> Result<Self, OnlinePacketPlanError> {
        if spec.sync_flag == 0
            || spec.locale_id <= 0
            || spec.initial_vendor_type < 0
            || spec.initial_register_type < 0
            || spec.status_vendor_type < 0
            || spec.status_register_type < 0
            || spec.auxiliary_flag == 0
            || spec.heartbeat_type <= 0
        {
            Err(OnlinePacketPlanError)
        } else {
            Ok(Self { spec })
        }
    }

    /// Returns the canonical numeric fields.
    #[must_use]
    pub const fn spec(self) -> OnlinePacketTuningSpec {
        self.spec
    }
}

/// Profile-selected routes and numeric values for compiled online codecs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OnlinePacketPlan {
    initial_sync_route: String,
    delayed_sync_route: String,
    status_register_route: Option<String>,
    heartbeat_route: String,
    tuning: OnlinePacketTuning,
}

/// Owned fields used to construct one online packet plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OnlinePacketPlanSpec {
    /// Route used by initial synchronization.
    pub initial_sync_route: String,
    /// Route used by server-requested delayed synchronization.
    pub delayed_sync_route: String,
    /// Optional standalone registration route.
    pub status_register_route: Option<String>,
    /// Route used by modern online heartbeats.
    pub heartbeat_route: String,
    /// Version-selected numeric packet values.
    pub tuning: OnlinePacketTuning,
}

impl OnlinePacketPlan {
    /// Creates a validated data-only plan for compiled packet codecs.
    ///
    /// # Errors
    ///
    /// Returns an error for empty, oversized or non-ASCII routes.
    pub fn new(spec: OnlinePacketPlanSpec) -> Result<Self, OnlinePacketPlanError> {
        if !valid_route(&spec.initial_sync_route)
            || !valid_route(&spec.delayed_sync_route)
            || spec
                .status_register_route
                .as_deref()
                .is_some_and(|route| !valid_route(route))
            || !valid_route(&spec.heartbeat_route)
        {
            return Err(OnlinePacketPlanError);
        }
        Ok(Self {
            initial_sync_route: spec.initial_sync_route,
            delayed_sync_route: spec.delayed_sync_route,
            status_register_route: spec.status_register_route,
            heartbeat_route: spec.heartbeat_route,
            tuning: spec.tuning,
        })
    }

    /// Returns the initial synchronization route.
    #[must_use]
    pub fn initial_sync_route(&self) -> &str {
        &self.initial_sync_route
    }

    /// Returns the delayed synchronization route.
    #[must_use]
    pub fn delayed_sync_route(&self) -> &str {
        &self.delayed_sync_route
    }

    /// Returns the optional standalone registration route.
    #[must_use]
    pub fn status_register_route(&self) -> Option<&str> {
        self.status_register_route.as_deref()
    }

    /// Returns the modern heartbeat route.
    #[must_use]
    pub fn heartbeat_route(&self) -> &str {
        &self.heartbeat_route
    }

    /// Returns the validated numeric packet values.
    #[must_use]
    pub const fn tuning(&self) -> OnlinePacketTuning {
        self.tuning
    }
}

/// Rejected signed Profile online packet plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OnlinePacketPlanError;

impl fmt::Display for OnlinePacketPlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("profile online packet plan rejected")
    }
}

impl std::error::Error for OnlinePacketPlanError {}

fn valid_route(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_ROUTE_LEN
        && value.bytes().all(|byte| byte.is_ascii_graphic())
}
