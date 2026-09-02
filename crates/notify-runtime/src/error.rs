/// Redacted notification model, storage, or worker failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NotificationError {
    /// Input or durable data violated a compiled invariant.
    Configuration,
    /// `SQLite` or filesystem persistence failed.
    Persistence,
    /// Operating-system randomness was unavailable.
    Identity,
    /// The requested outbox delivery does not exist or is already terminal.
    NotFound,
    /// The bounded notification worker is no longer accepting commands.
    Closed,
    /// The bounded notification queue is temporarily saturated.
    Busy,
    /// The notification task or blocking startup worker failed.
    Worker,
}

impl core::fmt::Display for NotificationError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::Configuration => "notification configuration rejected",
            Self::Persistence => "notification persistence failed",
            Self::Identity => "notification identity generation failed",
            Self::NotFound => "notification delivery is unavailable",
            Self::Closed => "notification runtime is closed",
            Self::Busy => "notification runtime queue is full",
            Self::Worker => "notification runtime worker failed",
        })
    }
}

impl std::error::Error for NotificationError {}

impl From<rusqlite::Error> for NotificationError {
    fn from(_error: rusqlite::Error) -> Self {
        Self::Persistence
    }
}

impl From<local_state::LocalStateError> for NotificationError {
    fn from(error: local_state::LocalStateError) -> Self {
        match error {
            local_state::LocalStateError::Configuration => Self::Configuration,
            local_state::LocalStateError::Persistence => Self::Persistence,
        }
    }
}
