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
    let group_record = MessageRecord::new(
        43,
        1_800_000_000_001,
        json!({"message_id": 43, "message": []}),
        RecallTarget::Group {
            group_code: 100,
            sequence: 101,
            random: Some(102),
        },
    )?;
    let mut store = MessageStore::open(&directory, local_id)?;
    store.put(&record)?;
    store.put(&group_record)?;
    drop(store);
    let reopened = MessageStore::open(&directory, local_id)?;
    assert_eq!(reopened.get(42)?, Some(record));
    assert_eq!(reopened.get(43)?, Some(group_record));
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

#[test]
fn schema_one_group_records_migrate_without_inventing_random() -> TestResult {
    let root = tempfile::tempdir()?;
    let directory = root.path().join("private");
    let local_id = AccountLocalId::from_bytes([11; 16]);
    let record = MessageRecord::new(
        9,
        10,
        json!({"message_id": 9, "message": []}),
        RecallTarget::Group {
            group_code: 100,
            sequence: 200,
            random: None,
        },
    )?;
    MessageStore::open(&directory, local_id)?.put(&record)?;
    let path = directory.join("messages-0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b.sqlite3");
    let connection = Connection::open(&path)?;
    connection.pragma_update(None, "user_version", 1)?;
    drop(connection);

    let reopened = MessageStore::open(&directory, local_id)?;
    assert_eq!(reopened.get(9)?, Some(record));
    let connection = Connection::open(path)?;
    assert_eq!(
        connection.query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))?,
        2
    );
    Ok(())
}
