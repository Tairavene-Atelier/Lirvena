//! Durable notification outbox, cooldown, and retry contract tests.

use notify_runtime::{
    DedupeKey, DestinationId, EventCategory, EventId, EventSource, EventState, FailureDisposition,
    NotificationError, NotificationEvent, NotificationStore, NotificationText, Severity,
    StateTransition,
};
use rusqlite::Connection;

const NOW_MS: u64 = 1_000_000;

#[test]
fn delivery_is_persisted_due_and_terminal() -> Result<(), Box<dyn std::error::Error>> {
    let temporary = tempfile::tempdir()?;
    let mut store = NotificationStore::open(&temporary.path().join("notify"))?;
    let destination = DestinationId::from_bytes([0x11; 16]);
    let event = event(
        1,
        EventCategory::Authorization,
        Severity::Critical,
        [0x21; 32],
    )?;
    assert_eq!(store.enqueue(&event, &[destination], NOW_MS)?, 1);

    let due = store.due(NOW_MS, 10)?;
    assert_eq!(due.len(), 1);
    assert_eq!(due[0].destination_id(), destination);
    assert_eq!(due[0].event(), &event);
    store.mark_delivered(due[0].id(), NOW_MS + 1)?;
    assert!(store.due(NOW_MS + 2, 10)?.is_empty());
    assert_eq!(
        store.mark_delivered(due[0].id(), NOW_MS + 3),
        Err(NotificationError::NotFound)
    );
    Ok(())
}

#[test]
fn cooldown_suppresses_duplicates_but_never_recovery() -> Result<(), Box<dyn std::error::Error>> {
    let temporary = tempfile::tempdir()?;
    let mut store = NotificationStore::open(&temporary.path().join("notify"))?;
    let destination = DestinationId::from_bytes([0x12; 16]);
    let first = event(1, EventCategory::Continuity, Severity::Warning, [0x22; 32])?;
    let duplicate = event(2, EventCategory::Continuity, Severity::Warning, [0x22; 32])?;
    let recovery = event(3, EventCategory::Recovery, Severity::Info, [0x22; 32])?;
    assert_eq!(store.enqueue(&first, &[destination], NOW_MS)?, 1);
    assert_eq!(store.enqueue(&duplicate, &[destination], NOW_MS + 1)?, 0);
    assert_eq!(store.enqueue(&recovery, &[destination], NOW_MS + 2)?, 1);
    assert_eq!(store.due(NOW_MS + 2, 10)?.len(), 2);
    Ok(())
}

#[test]
fn retries_follow_fixed_schedule_and_store_only_error_codes()
-> Result<(), Box<dyn std::error::Error>> {
    let temporary = tempfile::tempdir()?;
    let mut store = NotificationStore::open(&temporary.path().join("notify"))?;
    let event = event(1, EventCategory::Worker, Severity::Critical, [0x23; 32])?;
    store.enqueue(&event, &[DestinationId::from_bytes([0x13; 16])], NOW_MS)?;
    let delivery = store.due(NOW_MS, 1)?.remove(0);

    let first = NOW_MS + 60_000;
    assert_eq!(
        store.record_failure(delivery.id(), NOW_MS, 7)?,
        FailureDisposition::RetryAt(first)
    );
    assert!(store.due(first - 1, 1)?.is_empty());
    let retried = store.due(first, 1)?.remove(0);
    assert_eq!(retried.attempt_count(), 1);
    assert_eq!(
        store.record_failure(retried.id(), first, 8)?,
        FailureDisposition::RetryAt(first + 5 * 60_000)
    );
    Ok(())
}

#[test]
fn future_schema_is_rejected() -> Result<(), Box<dyn std::error::Error>> {
    let temporary = tempfile::tempdir()?;
    let directory = temporary.path().join("notify");
    drop(NotificationStore::open(&directory)?);
    let connection = Connection::open(directory.join("notifications.sqlite3"))?;
    connection.pragma_update(None, "user_version", 2)?;
    drop(connection);
    assert_eq!(
        NotificationStore::open(&directory).err(),
        Some(NotificationError::Configuration)
    );
    Ok(())
}

#[test]
fn debug_output_redacts_human_text_and_identifiers() -> Result<(), Box<dyn std::error::Error>> {
    let event = event(
        1,
        EventCategory::Authorization,
        Severity::Warning,
        [0x24; 32],
    )?;
    let debug = format!("{event:?}");
    assert!(!debug.contains("Authorization needs attention"));
    assert!(!debug.contains("Open Lirvena settings"));
    assert!(!debug.contains("24242424"));
    Ok(())
}

fn event(
    marker: u8,
    category: EventCategory,
    severity: Severity,
    dedupe: [u8; 32],
) -> Result<NotificationEvent, NotificationError> {
    let (previous, current) = if category == EventCategory::Recovery {
        (EventState::ProtectiveOffline, EventState::Active)
    } else {
        (EventState::Current, EventState::Unavailable)
    };
    NotificationEvent::new(
        EventId::from_bytes([marker; 16]),
        NOW_MS + u64::from(marker),
        EventSource::Ceylith,
        category,
        severity,
        Some([0x24; 16]),
        1,
        StateTransition::new(previous, current)?,
        NotificationText::new("Authorization needs attention")?,
        NotificationText::new("Open Lirvena settings")?,
        DedupeKey::from_bytes(dedupe),
    )
}
