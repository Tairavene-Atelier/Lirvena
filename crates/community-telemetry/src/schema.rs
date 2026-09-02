use rusqlite::Connection;

use crate::TelemetryStoreError;

pub(crate) fn migrate(connection: &mut Connection) -> Result<(), TelemetryStoreError> {
    let transaction = connection.transaction()?;
    transaction.execute_batch(
        "CREATE TABLE IF NOT EXISTS daily (
           utc_day INTEGER PRIMARY KEY,
           group_count INTEGER NOT NULL DEFAULT 0 CHECK (group_count >= 0),
           received INTEGER NOT NULL DEFAULT 0 CHECK (received >= 0),
           sent INTEGER NOT NULL DEFAULT 0 CHECK (sent >= 0),
           active_ms INTEGER NOT NULL DEFAULT 0 CHECK (active_ms >= 0),
           churn INTEGER NOT NULL DEFAULT 0 CHECK (churn >= 0),
           sent_at_ms INTEGER
         ) STRICT;
         CREATE TABLE IF NOT EXISTS runtime_state (
           singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
           active_accounts INTEGER NOT NULL CHECK (active_accounts >= 0),
           active_since_ms INTEGER,
           last_activity_ms INTEGER NOT NULL,
           account_set_digest BLOB
         ) STRICT;
         INSERT OR IGNORE INTO runtime_state
           (singleton, active_accounts, active_since_ms, last_activity_ms, account_set_digest)
           VALUES (1, 0, NULL, 0, NULL);",
    )?;
    transaction.commit()?;
    Ok(())
}
