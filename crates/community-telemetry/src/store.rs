use std::ffi::OsStr;
use std::path::Path;

use local_state::open_private_wal;
use rusqlite::{Connection, OptionalExtension, params};

use crate::model::{MILLIS_PER_DAY, to_completed_day, to_i64, to_u64, utc_day};
use crate::{CompletedDay, TelemetryStoreError};

/// Single-writer durable collector for one Lirvena installation.
pub struct CommunityTelemetryStore {
    connection: Connection,
}

impl CommunityTelemetryStore {
    /// Opens the private WAL store and closes any stale active interval at `opened_at_ms`.
    ///
    /// # Errors
    ///
    /// Returns an error for a zero timestamp or failed private persistence.
    pub fn open(directory: &Path, opened_at_ms: u64) -> Result<Self, TelemetryStoreError> {
        if opened_at_ms == 0 {
            return Err(TelemetryStoreError::InvalidInput);
        }
        let (_path, mut connection) =
            open_private_wal(directory, OsStr::new("community-telemetry.sqlite3"))?;
        crate::schema::migrate(&mut connection)?;
        close_stale_active_interval(&mut connection, opened_at_ms)?;
        Ok(Self { connection })
    }

    /// Records one received message without retaining its content or source.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid timestamp or persistence failure.
    pub fn record_received(&mut self, occurred_at_ms: u64) -> Result<(), TelemetryStoreError> {
        increment(&mut self.connection, occurred_at_ms, Counter::Received)
    }

    /// Records one successfully sent message without retaining its content or destination.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid timestamp or persistence failure.
    pub fn record_sent(&mut self, occurred_at_ms: u64) -> Result<(), TelemetryStoreError> {
        increment(&mut self.connection, occurred_at_ms, Counter::Sent)
    }

    /// Retains only the largest group-count snapshot observed during the UTC day.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid input or persistence failure.
    pub fn observe_group_count(
        &mut self,
        occurred_at_ms: u64,
        count: u64,
    ) -> Result<(), TelemetryStoreError> {
        let day = utc_day(occurred_at_ms)?;
        let count = to_i64(count)?;
        ensure_day(&mut self.connection, day)?;
        self.connection.execute(
            "UPDATE daily SET group_count = max(group_count, ?1) WHERE utc_day = ?2",
            params![count, day],
        )?;
        Ok(())
    }

    /// Reconciles the configured account set using a non-reversible, local-only digest.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid input or persistence failure.
    pub fn observe_account_set(
        &mut self,
        occurred_at_ms: u64,
        digest: [u8; 32],
    ) -> Result<(), TelemetryStoreError> {
        let day = utc_day(occurred_at_ms)?;
        ensure_day(&mut self.connection, day)?;
        let previous: Option<Vec<u8>> = self
            .connection
            .query_row(
                "SELECT account_set_digest FROM runtime_state WHERE singleton = 1",
                [],
                |row| row.get(0),
            )
            .optional()?
            .flatten();
        let changed = previous.as_deref().is_some_and(|value| value != digest);
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "UPDATE runtime_state SET account_set_digest = ?1 WHERE singleton = 1",
            params![digest.as_slice()],
        )?;
        if changed {
            transaction.execute(
                "UPDATE daily SET churn = churn + 1 WHERE utc_day = ?1",
                [day],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    /// Updates installation activity when one account enters or leaves `Active`.
    ///
    /// Duration is counted once while at least one account is active, never summed per account.
    ///
    /// # Errors
    ///
    /// Returns an error for a backward timestamp, unmatched leave, or persistence failure.
    pub fn set_account_active(
        &mut self,
        occurred_at_ms: u64,
        active: bool,
    ) -> Result<(), TelemetryStoreError> {
        update_activity(&mut self.connection, occurred_at_ms, active)
    }

    /// Persists elapsed activity without changing the number of active accounts.
    ///
    /// This is used at reporting boundaries so an account that stays online across midnight does
    /// not leave the completed day under-counted.
    ///
    /// # Errors
    ///
    /// Returns an error for a backward timestamp or persistence failure.
    pub fn checkpoint_activity(&mut self, occurred_at_ms: u64) -> Result<(), TelemetryStoreError> {
        let state = activity_state(&self.connection)?;
        if occurred_at_ms < state.last_activity_ms {
            return Err(TelemetryStoreError::InvalidInput);
        }
        if state.active_accounts > 0 {
            add_active_interval(&mut self.connection, state.active_since_ms, occurred_at_ms)?;
        }
        self.connection.execute(
            "UPDATE runtime_state SET active_since_ms = CASE WHEN active_accounts > 0 THEN ?1 \
             ELSE NULL END, last_activity_ms = ?1 WHERE singleton = 1",
            [to_i64(occurred_at_ms)?],
        )?;
        Ok(())
    }

    /// Returns the oldest unsent completed day, already reduced to approved buckets.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid input, corrupt values, or persistence failure.
    pub fn oldest_pending(&self, now_ms: u64) -> Result<Option<CompletedDay>, TelemetryStoreError> {
        let current_day = utc_day(now_ms)?;
        let raw = self
            .connection
            .query_row(
                "SELECT utc_day, group_count, received, sent, active_ms, churn FROM daily \
                 WHERE sent_at_ms IS NULL AND utc_day < ?1 ORDER BY utc_day LIMIT 1",
                [current_day],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, i64>(4)?,
                        row.get::<_, i64>(5)?,
                    ))
                },
            )
            .optional()?;
        raw.map(to_completed_day).transpose()
    }

    /// Marks an accepted report idempotently for one completed day.
    ///
    /// # Errors
    ///
    /// Returns an error for a current/future or unknown day, or persistence failure.
    pub fn mark_sent(
        &mut self,
        utc_day: u32,
        accepted_at_ms: u64,
    ) -> Result<(), TelemetryStoreError> {
        if utc_day >= crate::model::utc_day(accepted_at_ms)? {
            return Err(TelemetryStoreError::InvalidInput);
        }
        let changed = self.connection.execute(
            "UPDATE daily SET sent_at_ms = coalesce(sent_at_ms, ?1) WHERE utc_day = ?2",
            params![to_i64(accepted_at_ms)?, i64::from(utc_day)],
        )?;
        if changed == 1 {
            Ok(())
        } else {
            Err(TelemetryStoreError::InvalidInput)
        }
    }
}

#[derive(Clone, Copy)]
enum Counter {
    Received,
    Sent,
}

fn increment(
    connection: &mut Connection,
    occurred_at_ms: u64,
    counter: Counter,
) -> Result<(), TelemetryStoreError> {
    let day = utc_day(occurred_at_ms)?;
    ensure_day(connection, day)?;
    let statement = match counter {
        Counter::Received => "UPDATE daily SET received = received + 1 WHERE utc_day = ?1",
        Counter::Sent => "UPDATE daily SET sent = sent + 1 WHERE utc_day = ?1",
    };
    connection.execute(statement, [day])?;
    Ok(())
}

fn ensure_day(connection: &mut Connection, day: u32) -> Result<(), TelemetryStoreError> {
    connection.execute("INSERT OR IGNORE INTO daily (utc_day) VALUES (?1)", [day])?;
    Ok(())
}

struct ActivityState {
    active_accounts: u64,
    active_since_ms: Option<u64>,
    last_activity_ms: u64,
}

fn activity_state(connection: &Connection) -> Result<ActivityState, TelemetryStoreError> {
    let raw = connection.query_row(
        "SELECT active_accounts, active_since_ms, last_activity_ms FROM runtime_state \
         WHERE singleton = 1",
        [],
        |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, Option<i64>>(1)?,
                row.get::<_, i64>(2)?,
            ))
        },
    )?;
    Ok(ActivityState {
        active_accounts: to_u64(raw.0)?,
        active_since_ms: raw.1.map(to_u64).transpose()?,
        last_activity_ms: to_u64(raw.2)?,
    })
}

fn update_activity(
    connection: &mut Connection,
    occurred_at_ms: u64,
    active: bool,
) -> Result<(), TelemetryStoreError> {
    if occurred_at_ms == 0 {
        return Err(TelemetryStoreError::InvalidInput);
    }
    let state = activity_state(connection)?;
    if occurred_at_ms < state.last_activity_ms {
        return Err(TelemetryStoreError::InvalidInput);
    }
    let next = if active {
        state
            .active_accounts
            .checked_add(1)
            .ok_or(TelemetryStoreError::InvalidInput)?
    } else {
        state
            .active_accounts
            .checked_sub(1)
            .ok_or(TelemetryStoreError::InvalidInput)?
    };
    if !active && next == 0 {
        add_active_interval(connection, state.active_since_ms, occurred_at_ms)?;
    }
    let active_since = if active && state.active_accounts == 0 {
        Some(occurred_at_ms)
    } else if next == 0 {
        None
    } else {
        state.active_since_ms
    };
    connection.execute(
        "UPDATE runtime_state SET active_accounts = ?1, active_since_ms = ?2, last_activity_ms = ?3 \
         WHERE singleton = 1",
        params![
            to_i64(next)?,
            active_since.map(to_i64).transpose()?,
            to_i64(occurred_at_ms)?
        ],
    )?;
    Ok(())
}

fn close_stale_active_interval(
    connection: &mut Connection,
    opened_at_ms: u64,
) -> Result<(), TelemetryStoreError> {
    let state = activity_state(connection)?;
    if state.active_accounts > 0 {
        add_active_interval(connection, state.active_since_ms, opened_at_ms)?;
    }
    connection.execute(
        "UPDATE runtime_state SET active_accounts = 0, active_since_ms = NULL, last_activity_ms = ?1 \
         WHERE singleton = 1",
        [to_i64(opened_at_ms)?],
    )?;
    Ok(())
}

fn add_active_interval(
    connection: &mut Connection,
    started_at_ms: Option<u64>,
    ended_at_ms: u64,
) -> Result<(), TelemetryStoreError> {
    let start = started_at_ms.ok_or(TelemetryStoreError::Persistence)?;
    if ended_at_ms < start {
        return Err(TelemetryStoreError::InvalidInput);
    }
    let mut cursor = start;
    while cursor < ended_at_ms {
        let day = utc_day(cursor)?;
        let next_day = (u64::from(day) + 1)
            .checked_mul(MILLIS_PER_DAY)
            .ok_or(TelemetryStoreError::InvalidInput)?;
        let boundary = ended_at_ms.min(next_day);
        ensure_day(connection, day)?;
        connection.execute(
            "UPDATE daily SET active_ms = active_ms + ?1 WHERE utc_day = ?2",
            params![to_i64(boundary - cursor)?, day],
        )?;
        cursor = boundary;
    }
    Ok(())
}
