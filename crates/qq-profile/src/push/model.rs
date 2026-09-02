use core::fmt;
use std::collections::HashSet;

const MAX_ENTRIES: usize = 32;
const MAX_ROUTE_LEN: usize = 128;
const MAX_PUSH_BODY_LEN: u32 = 4 * 1024 * 1024;

/// Closed compiled behavior selected for one signed-Profile Push route.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum PushBehavior {
    /// Return the bounded body on the configured response route.
    EchoBody = 1,
    /// Decode two protobuf integer fields and acknowledge them on another route.
    ProtobufPairAck = 2,
    /// Parse a bounded configuration envelope and return its canonical acknowledgement.
    ConfigAck = 3,
    /// Accept the Push for bounded diagnostics without a response.
    Observe = 4,
    /// Force protective offline for the current transport generation.
    ProtectiveOffline = 5,
    /// Queue the body for the compiled message decoder.
    Message = 6,
    /// Parse the bounded legacy video envelope and conditionally acknowledge it.
    LegacyVideoAck = 7,
    /// Project a bounded synchronization Push into the current online generation.
    InfoSyncState = 8,
}

impl PushBehavior {
    pub(crate) const fn from_wire(value: u8) -> Result<Self, PushPlanError> {
        match value {
            1 => Ok(Self::EchoBody),
            2 => Ok(Self::ProtobufPairAck),
            3 => Ok(Self::ConfigAck),
            4 => Ok(Self::Observe),
            5 => Ok(Self::ProtectiveOffline),
            6 => Ok(Self::Message),
            7 => Ok(Self::LegacyVideoAck),
            8 => Ok(Self::InfoSyncState),
            _ => Err(PushPlanError),
        }
    }

    pub(crate) const fn needs_response_route(self) -> bool {
        matches!(
            self,
            Self::EchoBody | Self::ProtobufPairAck | Self::ConfigAck | Self::LegacyVideoAck
        )
    }
}

/// One validated route-to-primitive mapping from a signed Profile.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PushPlanEntry {
    route: String,
    behavior: PushBehavior,
    response_route: Option<String>,
    parameter: u32,
    maximum_body_len: u32,
}

impl PushPlanEntry {
    /// Creates a bounded data-only mapping to a compiled Push primitive.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid routes, response shape or body limit.
    pub fn new(
        route: String,
        behavior: PushBehavior,
        response_route: Option<String>,
        parameter: u32,
        maximum_body_len: u32,
    ) -> Result<Self, PushPlanError> {
        if !valid_route(&route)
            || response_route
                .as_deref()
                .is_some_and(|value| !valid_route(value))
            || behavior.needs_response_route() != response_route.is_some()
            || matches!(behavior, PushBehavior::ProtobufPairAck) != (parameter != 0)
            || maximum_body_len == 0
            || maximum_body_len > MAX_PUSH_BODY_LEN
        {
            return Err(PushPlanError);
        }
        Ok(Self {
            route,
            behavior,
            response_route,
            parameter,
            maximum_body_len,
        })
    }

    /// Returns the admitted inbound route.
    #[must_use]
    pub fn route(&self) -> &str {
        &self.route
    }

    /// Returns the selected compiled behavior.
    #[must_use]
    pub const fn behavior(&self) -> PushBehavior {
        self.behavior
    }

    /// Returns the response route when the primitive can acknowledge.
    #[must_use]
    pub fn response_route(&self) -> Option<&str> {
        self.response_route.as_deref()
    }

    /// Returns the numeric selector consumed by the chosen compiled primitive.
    #[must_use]
    pub const fn parameter(&self) -> u32 {
        self.parameter
    }

    /// Returns the Profile-selected body bound within the compiled ceiling.
    #[must_use]
    pub const fn maximum_body_len(&self) -> u32 {
        self.maximum_body_len
    }
}

/// Owned fields used to construct a signed-Profile Push plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PushPlanSpec {
    /// Unique route mappings.
    pub entries: Vec<PushPlanEntry>,
}

/// Validated Push admission and behavior plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PushPlan {
    entries: Box<[PushPlanEntry]>,
}

impl PushPlan {
    /// Validates plan size and unique routes.
    ///
    /// # Errors
    ///
    /// Returns an error for an empty, excessive or duplicate plan.
    pub fn new(spec: PushPlanSpec) -> Result<Self, PushPlanError> {
        if spec.entries.is_empty() || spec.entries.len() > MAX_ENTRIES {
            return Err(PushPlanError);
        }
        let mut routes = HashSet::with_capacity(spec.entries.len());
        if spec
            .entries
            .iter()
            .any(|entry| !routes.insert(entry.route.clone()))
        {
            return Err(PushPlanError);
        }
        Ok(Self {
            entries: spec.entries.into_boxed_slice(),
        })
    }

    /// Finds the primitive mapping for one authenticated route.
    #[must_use]
    pub fn find(&self, route: &str) -> Option<&PushPlanEntry> {
        self.entries.iter().find(|entry| entry.route == route)
    }

    /// Iterates over canonical entries.
    #[must_use]
    pub fn entries(&self) -> impl ExactSizeIterator<Item = &PushPlanEntry> {
        self.entries.iter()
    }
}

/// Rejected signed-Profile Push plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PushPlanError;

impl fmt::Display for PushPlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("profile Push plan rejected")
    }
}

impl std::error::Error for PushPlanError {}

fn valid_route(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_ROUTE_LEN
        && value.bytes().all(|byte| byte.is_ascii_graphic())
}
