use crate::AccountLocalId;

/// Durable account-worker lifecycle independent of QQ protocol sub-states.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum AccountPhase {
    /// No transport or account worker is active.
    Stopped = 0,
    /// A new isolated runtime generation is starting.
    Starting = 1,
    /// The current runtime generation completed all online gates.
    Active = 2,
    /// Operation stopped because a required safety property was lost.
    ProtectiveOffline = 3,
}

impl AccountPhase {
    pub(crate) const fn from_stored(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Stopped),
            1 => Some(Self::Starting),
            2 => Some(Self::Active),
            3 => Some(Self::ProtectiveOffline),
            _ => None,
        }
    }
}

/// Stable reason recorded whenever an account enters protective offline state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ProtectiveReason {
    /// The process ended while a runtime generation was active.
    ProcessRestart = 1,
    /// Required Ceylith continuity was lost.
    CeylithContinuity = 2,
    /// The active grant was revoked or expired.
    GrantUnavailable = 3,
    /// The selected Profile or its online plan became unusable.
    ProfileUnavailable = 4,
    /// QQ invalidated or removed the online session.
    RemoteSessionEnded = 5,
    /// An internal worker could not preserve its runtime invariants.
    WorkerFailure = 6,
}

impl ProtectiveReason {
    pub(crate) const fn from_stored(value: u8) -> Option<Self> {
        match value {
            1 => Some(Self::ProcessRestart),
            2 => Some(Self::CeylithContinuity),
            3 => Some(Self::GrantUnavailable),
            4 => Some(Self::ProfileUnavailable),
            5 => Some(Self::RemoteSessionEnded),
            6 => Some(Self::WorkerFailure),
            _ => None,
        }
    }
}

/// Latest durable state for one installation-local account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AccountSnapshot {
    local_id: AccountLocalId,
    phase: AccountPhase,
    generation: u64,
    protective_reason: Option<ProtectiveReason>,
    updated_at_ms: u64,
}

impl AccountSnapshot {
    pub(crate) const fn new(
        local_id: AccountLocalId,
        phase: AccountPhase,
        generation: u64,
        protective_reason: Option<ProtectiveReason>,
        updated_at_ms: u64,
    ) -> Self {
        Self {
            local_id,
            phase,
            generation,
            protective_reason,
            updated_at_ms,
        }
    }

    /// Returns the installation-local account identifier.
    #[must_use]
    pub const fn local_id(self) -> AccountLocalId {
        self.local_id
    }

    /// Returns the current durable lifecycle phase.
    #[must_use]
    pub const fn phase(self) -> AccountPhase {
        self.phase
    }

    /// Returns the monotonically increasing runtime generation.
    #[must_use]
    pub const fn generation(self) -> u64 {
        self.generation
    }

    /// Returns the reason for a protective stop, when applicable.
    #[must_use]
    pub const fn protective_reason(self) -> Option<ProtectiveReason> {
        self.protective_reason
    }

    /// Returns the persisted transition time in Unix milliseconds.
    #[must_use]
    pub const fn updated_at_ms(self) -> u64 {
        self.updated_at_ms
    }
}

/// Requested lifecycle transition sent to the single writer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AccountTransition {
    /// Destination phase.
    pub next: AccountPhase,
    /// Required only for `ProtectiveOffline`.
    pub protective_reason: Option<ProtectiveReason>,
    /// Caller-observed Unix time in milliseconds.
    pub occurred_at_ms: u64,
}

/// Durable transition result returned by the account actor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TransitionReceipt {
    previous: AccountSnapshot,
    current: AccountSnapshot,
}

impl TransitionReceipt {
    pub(crate) const fn new(previous: AccountSnapshot, current: AccountSnapshot) -> Self {
        Self { previous, current }
    }

    /// Returns the state before the committed transition.
    #[must_use]
    pub const fn previous(self) -> AccountSnapshot {
        self.previous
    }

    /// Returns the state after the committed transition.
    #[must_use]
    pub const fn current(self) -> AccountSnapshot {
        self.current
    }
}
