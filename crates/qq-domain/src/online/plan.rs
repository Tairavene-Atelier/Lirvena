use core::fmt;

use super::PlanActionId;

const MIN_COMPILED_INTERVAL_MS: u64 = 1_000;
const MAX_COMPILED_INTERVAL_MS: u64 = 24 * 60 * 60 * 1_000;

/// Validated, data-only online behavior supplied by a signed Ceylith Profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OnlinePlan {
    pub(super) initial_sync: PlanActionId,
    pub(super) delayed_sync: PlanActionId,
    pub(super) security_bootstrap: PlanActionId,
    pub(super) status_confirmation: Option<PlanActionId>,
    pub(super) business_heartbeat: PlanActionId,
    pub(super) initial_heartbeat_ms: u64,
    minimum_heartbeat_ms: u64,
    maximum_heartbeat_ms: u64,
    minimum_delayed_sync_ms: u64,
    maximum_delayed_sync_ms: u64,
}

/// Owned fields used to construct one bounded online plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OnlinePlanSpec {
    /// Initial synchronization action.
    pub initial_sync: PlanActionId,
    /// Server-requested continuation synchronization action.
    pub delayed_sync: PlanActionId,
    /// Required security bootstrap action.
    pub security_bootstrap: PlanActionId,
    /// Optional diagnostic status confirmation action.
    pub status_confirmation: Option<PlanActionId>,
    /// Business heartbeat action.
    pub business_heartbeat: PlanActionId,
    /// Initial heartbeat delay.
    pub initial_heartbeat_ms: u64,
    /// Smallest heartbeat delay accepted from a response.
    pub minimum_heartbeat_ms: u64,
    /// Largest heartbeat delay accepted from a response.
    pub maximum_heartbeat_ms: u64,
    /// Smallest delayed-sync delay accepted from a response.
    pub minimum_delayed_sync_ms: u64,
    /// Largest delayed-sync delay accepted from a response.
    pub maximum_delayed_sync_ms: u64,
}

impl OnlinePlan {
    /// Creates a bounded plan from opaque action identifiers and timing limits.
    ///
    /// Timing values remain profile data. The compiled bounds only prevent a
    /// remote plan from creating a busy loop or an unbounded stale schedule.
    ///
    /// # Errors
    ///
    /// Returns an error for duplicate actions or inconsistent timing bounds.
    pub fn new(spec: OnlinePlanSpec) -> Result<Self, OnlinePlanError> {
        let plan = Self {
            initial_sync: spec.initial_sync,
            delayed_sync: spec.delayed_sync,
            security_bootstrap: spec.security_bootstrap,
            status_confirmation: spec.status_confirmation,
            business_heartbeat: spec.business_heartbeat,
            initial_heartbeat_ms: spec.initial_heartbeat_ms,
            minimum_heartbeat_ms: spec.minimum_heartbeat_ms,
            maximum_heartbeat_ms: spec.maximum_heartbeat_ms,
            minimum_delayed_sync_ms: spec.minimum_delayed_sync_ms,
            maximum_delayed_sync_ms: spec.maximum_delayed_sync_ms,
        };
        plan.validate()?;
        Ok(plan)
    }

    /// Returns the canonical owned fields for profile serialization.
    #[must_use]
    pub const fn spec(self) -> OnlinePlanSpec {
        OnlinePlanSpec {
            initial_sync: self.initial_sync,
            delayed_sync: self.delayed_sync,
            security_bootstrap: self.security_bootstrap,
            status_confirmation: self.status_confirmation,
            business_heartbeat: self.business_heartbeat,
            initial_heartbeat_ms: self.initial_heartbeat_ms,
            minimum_heartbeat_ms: self.minimum_heartbeat_ms,
            maximum_heartbeat_ms: self.maximum_heartbeat_ms,
            minimum_delayed_sync_ms: self.minimum_delayed_sync_ms,
            maximum_delayed_sync_ms: self.maximum_delayed_sync_ms,
        }
    }

    pub(super) fn clamp_heartbeat(self, requested_ms: u64) -> u64 {
        requested_ms.clamp(self.minimum_heartbeat_ms, self.maximum_heartbeat_ms)
    }

    pub(super) fn clamp_delayed_sync(self, requested_ms: u64) -> u64 {
        requested_ms.clamp(self.minimum_delayed_sync_ms, self.maximum_delayed_sync_ms)
    }

    fn validate(self) -> Result<(), OnlinePlanError> {
        let ids = [
            Some(self.initial_sync),
            Some(self.delayed_sync),
            Some(self.security_bootstrap),
            self.status_confirmation,
            Some(self.business_heartbeat),
        ];
        for (index, id) in ids.iter().enumerate() {
            if id.is_some() && ids[..index].contains(id) {
                return Err(OnlinePlanError);
            }
        }
        validate_range(
            self.minimum_heartbeat_ms,
            self.initial_heartbeat_ms,
            self.maximum_heartbeat_ms,
        )?;
        validate_range(
            self.minimum_delayed_sync_ms,
            self.minimum_delayed_sync_ms,
            self.maximum_delayed_sync_ms,
        )
    }
}

fn validate_range(minimum: u64, value: u64, maximum: u64) -> Result<(), OnlinePlanError> {
    if minimum < MIN_COMPILED_INTERVAL_MS
        || maximum > MAX_COMPILED_INTERVAL_MS
        || minimum > value
        || value > maximum
    {
        Err(OnlinePlanError)
    } else {
        Ok(())
    }
}

/// Rejected signed Profile online plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OnlinePlanError;

impl fmt::Display for OnlinePlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("online plan rejected")
    }
}

impl std::error::Error for OnlinePlanError {}
