//! Durable Community aggregation tests.

use ceylith_protocol::{
    AccountChurnBucket, ActiveDurationBucket, GroupCountBucket, MessageCountBucket,
};
use community_telemetry::CommunityTelemetryStore;

const DAY: u64 = 86_400_000;

#[test]
fn completed_day_exposes_only_fixed_buckets() -> Result<(), Box<dyn std::error::Error>> {
    let temporary = tempfile::tempdir()?;
    let directory = temporary.path().join("state");
    let mut store = CommunityTelemetryStore::open(&directory, DAY + 1_000)?;
    store.observe_account_set(DAY + 1_000, [1_u8; 32])?;
    store.observe_account_set(DAY + 2_000, [2_u8; 32])?;
    store.observe_group_count(DAY + 3_000, 73)?;
    for offset in 0..101 {
        store.record_received(DAY + 4_000 + offset)?;
    }
    for offset in 0..21 {
        store.record_sent(DAY + 5_000 + offset)?;
    }
    store.set_account_active(DAY + 6_000, true)?;
    store.set_account_active(DAY + 6_000 + 3_600_000, false)?;

    let report = store
        .oldest_pending(2 * DAY + 1_000)?
        .ok_or("completed day missing")?;
    assert_eq!(report.utc_day, 1);
    assert_eq!(report.group_count, GroupCountBucket::FiftyOneToOneHundred);
    assert_eq!(
        report.messages_received,
        MessageCountBucket::OneHundredOneToFiveHundred
    );
    assert_eq!(
        report.messages_sent,
        MessageCountBucket::TwentyOneToOneHundred
    );
    assert_eq!(report.active_duration, ActiveDurationBucket::OneToFourHours);
    assert_eq!(report.account_churn, AccountChurnBucket::One);
    Ok(())
}

#[test]
fn active_duration_is_not_summed_per_account_and_splits_at_utc_boundary()
-> Result<(), Box<dyn std::error::Error>> {
    let temporary = tempfile::tempdir()?;
    let mut store = CommunityTelemetryStore::open(temporary.path(), DAY - 2_000)?;
    store.set_account_active(DAY - 1_000, true)?;
    store.set_account_active(DAY - 500, true)?;
    store.set_account_active(DAY + 1_000, false)?;
    store.set_account_active(DAY + 2_000, false)?;

    let first = store
        .oldest_pending(DAY + 3_000)?
        .ok_or("first day missing")?;
    assert_eq!(first.utc_day, 0);
    assert_eq!(first.active_duration, ActiveDurationBucket::UnderOneHour);
    store.mark_sent(first.utc_day, DAY + 3_000)?;
    assert!(store.oldest_pending(DAY + 3_000)?.is_none());
    Ok(())
}

#[test]
fn current_day_cannot_be_reported_or_marked_sent() -> Result<(), Box<dyn std::error::Error>> {
    let temporary = tempfile::tempdir()?;
    let mut store = CommunityTelemetryStore::open(temporary.path(), DAY + 1)?;
    store.record_received(DAY + 2)?;
    assert!(store.oldest_pending(DAY + 3)?.is_none());
    assert!(store.mark_sent(1, DAY + 3).is_err());
    Ok(())
}

#[test]
fn checkpoint_closes_previous_day_while_account_stays_active()
-> Result<(), Box<dyn std::error::Error>> {
    let temporary = tempfile::tempdir()?;
    let mut store = CommunityTelemetryStore::open(temporary.path(), DAY - 2_000)?;
    store.set_account_active(DAY - 1_000, true)?;
    store.checkpoint_activity(DAY + 1_000)?;
    let first = store
        .oldest_pending(DAY + 1_000)?
        .ok_or("previous day missing")?;
    assert_eq!(first.utc_day, 0);
    assert_eq!(first.active_duration, ActiveDurationBucket::UnderOneHour);
    store.set_account_active(DAY + 2_000, false)?;
    Ok(())
}
