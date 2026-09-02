//! Durable bounded account message-state contracts.

use account_message_store::{MessageRecord, MessageStore, RecallTarget};
use account_runtime::AccountLocalId;
use rusqlite::Connection;
use serde_json::json;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn records_survive_restart_and_recall_fields_round_trip() -> TestResult {
    let root = tempfile::tempdir()?;
    let directory = root.path().join("private");
    let local_id = AccountLocalId::from_bytes([7; 16]);
    let record = MessageRecord::new(
        42,
        1_800_000_000_000,
        json!({"message_id": 42, "message": []}),
        RecallTarget::Private {
            uid: "u_peer".to_owned(),
            sequence: u64::MAX,
            client_sequence: 700,
            random: 99,
            timestamp: 1_800_000_000,
        },
    )?;
    MessageStore::open(&directory, local_id)?.put(&record)?;
    let reopened = MessageStore::open(&directory, local_id)?;
    assert_eq!(reopened.get(42)?, Some(record));
    Ok(())
}

#[test]
fn accounts_are_isolated_and_removal_is_durable() -> TestResult {
    let root = tempfile::tempdir()?;
    let directory = root.path().join("private");
    let local_id = AccountLocalId::from_bytes([8; 16]);
    let mut store = MessageStore::open(&directory, local_id)?;
    assert!(
        MessageRecord::new(
            1,
            1,
            json!({"message": "x".repeat(70_000)}),
            RecallTarget::Unavailable,
        )
        .is_err()
    );
    assert!(!store.contains(1)?);
    let record = MessageRecord::new(
        7,
        10,
        json!({"message_id": 7, "message": []}),
        RecallTarget::Unavailable,
    )?;
    store.put(&record)?;
    assert!(store.contains(7)?);
    assert!(!MessageStore::open(&directory, AccountLocalId::from_bytes([9; 16]))?.contains(7)?);
    store.remove(7)?;
    drop(store);
    assert!(!MessageStore::open(&directory, local_id)?.contains(7)?);
    Ok(())
}

#[test]
fn unknown_schema_fails_closed() -> TestResult {
    let root = tempfile::tempdir()?;
    let directory = root.path().join("private");
    let local_id = AccountLocalId::from_bytes([10; 16]);
    drop(MessageStore::open(&directory, local_id)?);
    let path = directory.join("messages-0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a.sqlite3");
    let connection = Connection::open(path)?;
    connection.pragma_update(None, "user_version", 99)?;
    drop(connection);
    assert!(MessageStore::open(&directory, local_id).is_err());
    Ok(())
}
