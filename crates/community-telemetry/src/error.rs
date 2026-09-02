/// Durable aggregation failure with no sensitive values in its representation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TelemetryStoreError {
    /// A timestamp or transition violated the closed state model.
    InvalidInput,
    /// Private local persistence failed.
    Persistence,
}

impl core::fmt::Display for TelemetryStoreError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::InvalidInput => "Community telemetry input rejected",
            Self::Persistence => "Community telemetry persistence failed",
        })
    }
}

impl std::error::Error for TelemetryStoreError {}

impl From<rusqlite::Error> for TelemetryStoreError {
    fn from(_error: rusqlite::Error) -> Self {
        Self::Persistence
    }
}

impl From<local_state::LocalStateError> for TelemetryStoreError {
    fn from(_error: local_state::LocalStateError) -> Self {
        Self::Persistence
    }
}
