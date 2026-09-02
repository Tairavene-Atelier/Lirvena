use core::fmt;

/// Redacted account actor or persistence failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AccountRuntimeError {
    /// Runtime configuration or durable bytes were invalid.
    Configuration,
    /// `SQLite` could not open, validate or commit account state.
    Persistence,
    /// The account actor is no longer accepting commands.
    Closed,
    /// The installation-local account already has a runtime.
    DuplicateAccount,
    /// No runtime exists for the installation-local account.
    UnknownAccount,
    /// The requested lifecycle transition is not allowed.
    TransitionRejected,
    /// The account actor thread ended unexpectedly.
    WorkerFailed,
}

impl fmt::Display for AccountRuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Configuration => "account runtime configuration rejected",
            Self::Persistence => "account state persistence failed",
            Self::Closed => "account runtime is closed",
            Self::DuplicateAccount => "account runtime already exists",
            Self::UnknownAccount => "account runtime does not exist",
            Self::TransitionRejected => "account lifecycle transition rejected",
            Self::WorkerFailed => "account runtime worker failed",
        })
    }
}

impl std::error::Error for AccountRuntimeError {}

impl From<rusqlite::Error> for AccountRuntimeError {
    fn from(_error: rusqlite::Error) -> Self {
        Self::Persistence
    }
}

impl From<local_state::LocalStateError> for AccountRuntimeError {
    fn from(error: local_state::LocalStateError) -> Self {
        match error {
            local_state::LocalStateError::Configuration => Self::Configuration,
            local_state::LocalStateError::Persistence => Self::Persistence,
        }
    }
}
