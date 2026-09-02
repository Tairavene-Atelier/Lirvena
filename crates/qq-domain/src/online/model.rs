use core::fmt;

use super::OnlinePlanError;

/// Opaque identifier for one action admitted by a Ceylith online plan.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct PlanActionId([u8; 16]);

impl PlanActionId {
    /// Creates a non-zero action identifier.
    ///
    /// # Errors
    ///
    /// Returns an error when `bytes` is the reserved all-zero identifier.
    pub fn new(bytes: [u8; 16]) -> Result<Self, OnlinePlanError> {
        if bytes == [0; 16] {
            Err(OnlinePlanError)
        } else {
            Ok(Self(bytes))
        }
    }

    /// Returns the opaque bytes without assigning protocol semantics to them.
    #[must_use]
    pub const fn as_bytes(self) -> [u8; 16] {
        self.0
    }
}

/// One closed action class understood by the compiled Lirvena runtime.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OnlineAction {
    /// Initial synchronization for a new online generation.
    InitialSync(PlanActionId),
    /// Server-requested continuation of synchronization.
    DelayedSync(PlanActionId),
    /// Required security-chain bootstrap.
    SecurityBootstrap(PlanActionId),
    /// Optional status confirmation after security bootstrap.
    StatusConfirmation(PlanActionId),
    /// Online business heartbeat.
    BusinessHeartbeat(PlanActionId),
}

impl OnlineAction {
    pub(super) const fn id(self) -> PlanActionId {
        match self {
            Self::InitialSync(id)
            | Self::DelayedSync(id)
            | Self::SecurityBootstrap(id)
            | Self::StatusConfirmation(id)
            | Self::BusinessHeartbeat(id) => id,
        }
    }
}

/// Monotonic identifier for one account online generation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct OnlineGeneration(u64);

impl OnlineGeneration {
    /// Creates a non-zero generation identifier.
    ///
    /// # Errors
    ///
    /// Returns an error for the reserved zero generation.
    pub const fn new(value: u64) -> Result<Self, OnlineTransitionError> {
        if value == 0 {
            Err(OnlineTransitionError)
        } else {
            Ok(Self(value))
        }
    }

    /// Returns the numeric generation.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Closed state of one online-generation controller.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OnlineState {
    /// No online generation owns the transport.
    Stopped,
    /// Initial synchronization is in flight.
    Synchronizing(OnlineGeneration),
    /// Required security bootstrap is in flight.
    Bootstrapping(OnlineGeneration),
    /// Optional status confirmation is in flight.
    Confirming(OnlineGeneration),
    /// The generation is online and its schedules are active.
    Online(OnlineGeneration),
    /// Required continuity failed and the transport must remain closed.
    ProtectiveOffline(OnlineGeneration),
}

/// Side effect requested from the account's single-writer actor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OnlineDirective {
    /// Dispatch one opaque action through its compiled adapter.
    Dispatch {
        /// Online generation that owns the action.
        generation: OnlineGeneration,
        /// Closed action class plus opaque plan identifier.
        action: OnlineAction,
    },
    /// The generation completed all required startup gates.
    EnteredOnline(OnlineGeneration),
    /// Close the QQ transport without discarding durable account state.
    ProtectiveOffline(OnlineGeneration),
}

/// Rejected state transition or stale action completion.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OnlineTransitionError;

impl fmt::Display for OnlineTransitionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("online state transition rejected")
    }
}

impl std::error::Error for OnlineTransitionError {}
