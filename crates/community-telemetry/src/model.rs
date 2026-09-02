use ceylith_protocol::{
    AccountChurnBucket, ActiveDurationBucket, GroupCountBucket, MessageCountBucket,
};

use crate::TelemetryStoreError;

pub(crate) const MILLIS_PER_DAY: u64 = 86_400_000;

/// Approved coarse summary for one completed UTC day.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompletedDay {
    /// UTC day number since the Unix epoch.
    pub utc_day: u32,
    /// Coarse maximum observed group count.
    pub group_count: GroupCountBucket,
    /// Coarse received-message count.
    pub messages_received: MessageCountBucket,
    /// Coarse sent-message count.
    pub messages_sent: MessageCountBucket,
    /// Coarse duration with at least one active account.
    pub active_duration: ActiveDurationBucket,
    /// Coarse configured-account-set churn.
    pub account_churn: AccountChurnBucket,
}

pub(crate) fn to_completed_day(
    raw: (i64, i64, i64, i64, i64, i64),
) -> Result<CompletedDay, TelemetryStoreError> {
    Ok(CompletedDay {
        utc_day: u32::try_from(raw.0).map_err(|_error| TelemetryStoreError::Persistence)?,
        group_count: GroupCountBucket::from_count(to_u64(raw.1)?),
        messages_received: MessageCountBucket::from_count(to_u64(raw.2)?),
        messages_sent: MessageCountBucket::from_count(to_u64(raw.3)?),
        active_duration: ActiveDurationBucket::from_milliseconds(to_u64(raw.4)?),
        account_churn: AccountChurnBucket::from_count(to_u64(raw.5)?),
    })
}

pub(crate) fn utc_day(timestamp_ms: u64) -> Result<u32, TelemetryStoreError> {
    if timestamp_ms == 0 {
        return Err(TelemetryStoreError::InvalidInput);
    }
    u32::try_from(timestamp_ms / MILLIS_PER_DAY).map_err(|_error| TelemetryStoreError::InvalidInput)
}

pub(crate) fn to_i64(value: u64) -> Result<i64, TelemetryStoreError> {
    i64::try_from(value).map_err(|_error| TelemetryStoreError::InvalidInput)
}

pub(crate) fn to_u64(value: i64) -> Result<u64, TelemetryStoreError> {
    u64::try_from(value).map_err(|_error| TelemetryStoreError::Persistence)
}
