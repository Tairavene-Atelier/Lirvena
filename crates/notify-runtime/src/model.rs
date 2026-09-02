use crate::{DedupeKey, EventId, NotificationError};

const MAX_TEXT_BYTES: usize = 512;

/// Closed origin for an operational event.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum EventSource {
    /// Ceylith authorization or online-plan continuity.
    Ceylith = 1,
    /// QQ remote transport or account state.
    Qq = 2,
    /// Lirvena account worker supervision.
    Account = 3,
    /// Lirvena local runtime infrastructure.
    Lirvena = 4,
}

/// Closed operational category used for routing and cooldown policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum EventCategory {
    /// Token, renewal, quota, or policy authorization.
    Authorization = 1,
    /// Ceylith, Profile, or online-plan continuity.
    Continuity = 2,
    /// QQ kick, verification, suspected ban, or risk-control signal.
    RiskControl = 3,
    /// Account worker crash, restart loop, or queue pressure.
    Worker = 4,
    /// Explicit recovery from a prior degraded state.
    Recovery = 5,
}

/// Closed urgency used for delivery retry lifetime.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Severity {
    /// Informational state change.
    Info = 1,
    /// User attention is required soon.
    Warning = 2,
    /// Safety or account availability requires immediate attention.
    Critical = 3,
}

/// Closed user-visible state names shared by all notification adapters.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum EventState {
    /// Normal current state.
    Current = 1,
    /// Authorization or lease is approaching expiry.
    Expiring = 2,
    /// Automatic continuation is paused.
    Paused = 3,
    /// Authorization was explicitly revoked.
    Revoked = 4,
    /// Required capability or service is unavailable.
    Unavailable = 5,
    /// Account transport was closed for safety.
    ProtectiveOffline = 6,
    /// Runtime is attempting a bounded recovery.
    Recovering = 7,
    /// Runtime is active.
    Active = 8,
    /// Runtime is intentionally stopped.
    Stopped = 9,
    /// Operation or worker failed.
    Failed = 10,
}

macro_rules! stored_enum {
    ($name:ident { $($value:ident),+ $(,)? }) => {
        impl $name {
            pub(crate) fn from_stored(value: u8) -> Result<Self, NotificationError> {
                match value {
                    $(value if value == Self::$value as u8 => Ok(Self::$value),)+
                    _ => Err(NotificationError::Configuration),
                }
            }
        }
    };
}

stored_enum!(EventSource {
    Ceylith,
    Qq,
    Account,
    Lirvena
});
stored_enum!(EventCategory {
    Authorization,
    Continuity,
    RiskControl,
    Worker,
    Recovery,
});
stored_enum!(Severity {
    Info,
    Warning,
    Critical
});
stored_enum!(EventState {
    Current,
    Expiring,
    Paused,
    Revoked,
    Unavailable,
    ProtectiveOffline,
    Recovering,
    Active,
    Stopped,
    Failed,
});

/// Bounded user-facing text; Debug output is always redacted.
#[derive(Clone, Eq, PartialEq)]
pub struct NotificationText(Box<str>);

impl NotificationText {
    /// Validates bounded UTF-8 text without carriage returns or terminal control characters.
    ///
    /// # Errors
    ///
    /// Returns an error for empty, oversized, or control-bearing text.
    pub fn new(value: impl Into<String>) -> Result<Self, NotificationError> {
        let value = value.into();
        if value.is_empty()
            || value.len() > MAX_TEXT_BYTES
            || value
                .chars()
                .any(|character| character == '\r' || (character.is_control() && character != '\n'))
        {
            return Err(NotificationError::Configuration);
        }
        Ok(Self(value.into_boxed_str()))
    }

    /// Borrows the validated text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl core::fmt::Debug for NotificationText {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("NotificationText(<redacted>)")
    }
}

/// Explicit state change rendered consistently by every adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StateTransition {
    previous: EventState,
    current: EventState,
}

impl StateTransition {
    /// Creates a non-self transition.
    ///
    /// # Errors
    ///
    /// Returns an error when both states are equal.
    pub fn new(previous: EventState, current: EventState) -> Result<Self, NotificationError> {
        if previous == current {
            Err(NotificationError::Configuration)
        } else {
            Ok(Self { previous, current })
        }
    }

    #[must_use]
    /// Returns the previous closed state.
    pub const fn previous(self) -> EventState {
        self.previous
    }

    #[must_use]
    /// Returns the current closed state.
    pub const fn current(self) -> EventState {
        self.current
    }
}

/// Validated canonical event persisted once and delivered to one or more destinations.
#[derive(Clone, Eq, PartialEq)]
pub struct NotificationEvent {
    event_id: EventId,
    occurred_at_ms: u64,
    source: EventSource,
    category: EventCategory,
    severity: Severity,
    account_local_id: Option<[u8; 16]>,
    reason_code: u32,
    transition: StateTransition,
    human_summary: NotificationText,
    next_action: NotificationText,
    dedupe_key: DedupeKey,
}

impl NotificationEvent {
    /// Creates a canonical notification event.
    ///
    /// # Errors
    ///
    /// Returns an error when time or reason code is zero.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        event_id: EventId,
        occurred_at_ms: u64,
        source: EventSource,
        category: EventCategory,
        severity: Severity,
        account_local_id: Option<[u8; 16]>,
        reason_code: u32,
        transition: StateTransition,
        human_summary: NotificationText,
        next_action: NotificationText,
        dedupe_key: DedupeKey,
    ) -> Result<Self, NotificationError> {
        if occurred_at_ms == 0 || reason_code == 0 {
            return Err(NotificationError::Configuration);
        }
        Ok(Self {
            event_id,
            occurred_at_ms,
            source,
            category,
            severity,
            account_local_id,
            reason_code,
            transition,
            human_summary,
            next_action,
            dedupe_key,
        })
    }

    #[must_use]
    /// Returns the event identifier.
    pub const fn event_id(&self) -> EventId {
        self.event_id
    }

    #[must_use]
    /// Returns the event occurrence time in Unix milliseconds.
    pub const fn occurred_at_ms(&self) -> u64 {
        self.occurred_at_ms
    }

    #[must_use]
    /// Returns the event source.
    pub const fn source(&self) -> EventSource {
        self.source
    }

    #[must_use]
    /// Returns the routing category.
    pub const fn category(&self) -> EventCategory {
        self.category
    }

    #[must_use]
    /// Returns the delivery severity.
    pub const fn severity(&self) -> Severity {
        self.severity
    }

    #[must_use]
    /// Returns the optional opaque installation-local account identifier.
    pub const fn account_local_id(&self) -> Option<&[u8; 16]> {
        self.account_local_id.as_ref()
    }

    #[must_use]
    /// Returns the stable code within the event category.
    pub const fn reason_code(&self) -> u32 {
        self.reason_code
    }

    #[must_use]
    /// Returns the explicit state transition.
    pub const fn transition(&self) -> StateTransition {
        self.transition
    }

    #[must_use]
    /// Returns the bounded user-facing summary.
    pub const fn human_summary(&self) -> &NotificationText {
        &self.human_summary
    }

    #[must_use]
    /// Returns the bounded user-facing next action.
    pub const fn next_action(&self) -> &NotificationText {
        &self.next_action
    }

    #[must_use]
    /// Returns the cooldown equivalence key.
    pub const fn dedupe_key(&self) -> DedupeKey {
        self.dedupe_key
    }
}

impl core::fmt::Debug for NotificationEvent {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("NotificationEvent")
            .field("event_id", &self.event_id)
            .field("occurred_at_ms", &self.occurred_at_ms)
            .field("source", &self.source)
            .field("category", &self.category)
            .field("severity", &self.severity)
            .field("account_scoped", &self.account_local_id.is_some())
            .field("reason_code", &self.reason_code)
            .field("transition", &self.transition)
            .field("human_summary", &"<redacted>")
            .field("next_action", &"<redacted>")
            .field("dedupe_key", &self.dedupe_key)
            .finish()
    }
}
