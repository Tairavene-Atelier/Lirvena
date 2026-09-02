use rusqlite::Connection;

use super::SCHEMA_VERSION;
use crate::NotificationError;

pub(super) fn migrate(connection: &mut Connection) -> Result<(), NotificationError> {
    let version: u32 = connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    match version {
        SCHEMA_VERSION => Ok(()),
        0 => create_schema(connection),
        _ => Err(NotificationError::Configuration),
    }
}

fn create_schema(connection: &mut Connection) -> Result<(), NotificationError> {
    let transaction = connection.transaction()?;
    transaction.execute_batch(
        "CREATE TABLE notification_event ( \
            event_id BLOB PRIMARY KEY CHECK (length(event_id) = 16), \
            occurred_at_ms INTEGER NOT NULL CHECK (occurred_at_ms > 0), \
            enqueued_at_ms INTEGER NOT NULL CHECK (enqueued_at_ms >= 0), \
            source INTEGER NOT NULL, category INTEGER NOT NULL, severity INTEGER NOT NULL, \
            account_local_id BLOB CHECK (account_local_id IS NULL OR length(account_local_id) = 16), \
            reason_code INTEGER NOT NULL CHECK (reason_code > 0), \
            previous_state INTEGER NOT NULL, current_state INTEGER NOT NULL, \
            human_summary TEXT NOT NULL, next_action TEXT NOT NULL, \
            dedupe_key BLOB NOT NULL CHECK (length(dedupe_key) = 32) \
         ); \
         CREATE TABLE notification_delivery ( \
            id INTEGER PRIMARY KEY AUTOINCREMENT, \
            event_id BLOB NOT NULL REFERENCES notification_event(event_id) ON DELETE RESTRICT, \
            destination_id BLOB NOT NULL CHECK (length(destination_id) = 16), \
            attempt_count INTEGER NOT NULL CHECK (attempt_count >= 0), \
            next_attempt_at_ms INTEGER NOT NULL CHECK (next_attempt_at_ms >= 0), \
            retry_until_ms INTEGER NOT NULL CHECK (retry_until_ms >= next_attempt_at_ms), \
            delivered_at_ms INTEGER, abandoned_at_ms INTEGER, last_error_code INTEGER, \
            UNIQUE (event_id, destination_id), \
            CHECK (NOT (delivered_at_ms IS NOT NULL AND abandoned_at_ms IS NOT NULL)) \
         ); \
         CREATE INDEX notification_due_idx ON notification_delivery \
            (next_attempt_at_ms, id) WHERE delivered_at_ms IS NULL AND abandoned_at_ms IS NULL; \
         CREATE INDEX notification_dedupe_idx ON notification_event \
            (dedupe_key, enqueued_at_ms);",
    )?;
    transaction.pragma_update(None, "user_version", SCHEMA_VERSION)?;
    transaction.commit()?;
    Ok(())
}
