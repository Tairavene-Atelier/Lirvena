use rusqlite::{Connection, params};

use crate::{AccountPhase, AccountRuntimeError};

const SCHEMA_VERSION: u32 = 1;

pub(super) fn migrate(
    connection: &mut Connection,
    created_at_ms: u64,
) -> Result<(), AccountRuntimeError> {
    let version: u32 = connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    match version {
        SCHEMA_VERSION => Ok(()),
        0 => create_schema(connection, created_at_ms),
        _ => Err(AccountRuntimeError::Configuration),
    }
}

fn create_schema(
    connection: &mut Connection,
    created_at_ms: u64,
) -> Result<(), AccountRuntimeError> {
    let created_at_ms =
        i64::try_from(created_at_ms).map_err(|_error| AccountRuntimeError::Configuration)?;
    let transaction = connection.transaction()?;
    transaction.execute_batch(
        "CREATE TABLE account_state ( \
            singleton INTEGER PRIMARY KEY CHECK (singleton = 1), \
            phase INTEGER NOT NULL, \
            generation INTEGER NOT NULL CHECK (generation >= 0), \
            protective_reason INTEGER, \
            updated_at_ms INTEGER NOT NULL CHECK (updated_at_ms >= 0) \
         ); \
         CREATE TABLE account_transitions ( \
            id INTEGER PRIMARY KEY AUTOINCREMENT, \
            generation INTEGER NOT NULL CHECK (generation >= 0), \
            from_phase INTEGER NOT NULL, \
            to_phase INTEGER NOT NULL, \
            protective_reason INTEGER, \
            occurred_at_ms INTEGER NOT NULL CHECK (occurred_at_ms >= 0) \
         );",
    )?;
    transaction.execute(
        "INSERT INTO account_state \
         (singleton, phase, generation, protective_reason, updated_at_ms) \
         VALUES (1, ?1, 0, NULL, ?2)",
        params![AccountPhase::Stopped as u8, created_at_ms],
    )?;
    transaction.pragma_update(None, "user_version", SCHEMA_VERSION)?;
    transaction.commit().map_err(Into::into)
}
