use std::ffi::OsStr;
use std::path::Path;

use local_state::open_private_wal;
use rusqlite::{Connection, OptionalExtension, params};

use crate::{DeliveryId, DestinationId, NotificationError, NotificationEvent};

mod codec;
mod policy;
mod schema;

use codec::{decode_delivery, to_i64};
use policy::{eligible_destinations, retry_delay, retry_lifetime, validate_destinations};
use schema::migrate;

const DATABASE_NAME: &str = "notifications.sqlite3";
const SCHEMA_VERSION: u32 = 1;
const DELIVERY_BATCH_LIMIT: usize = 100;

/// One pending delivery and its canonical event.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Delivery {
    id: DeliveryId,
    destination_id: DestinationId,
    attempt_count: u32,
    event: NotificationEvent,
}

impl Delivery {
    /// Returns the local outbox delivery identifier.
    #[must_use]
    pub const fn id(&self) -> DeliveryId {
        self.id
    }

    /// Returns the configured destination identifier.
    #[must_use]
    pub const fn destination_id(&self) -> DestinationId {
        self.destination_id
    }

    /// Returns the number of previously recorded failures.
    #[must_use]
    pub const fn attempt_count(&self) -> u32 {
        self.attempt_count
    }

    /// Returns the immutable canonical event.
    #[must_use]
    pub const fn event(&self) -> &NotificationEvent {
        &self.event
    }
}

/// Result of recording one failed delivery attempt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FailureDisposition {
    /// Delivery remains pending until the returned Unix time.
    RetryAt(u64),
    /// Delivery reached its severity-specific retry lifetime.
    Abandoned,
}

/// Single-writer durable notification outbox.
pub struct NotificationStore {
    connection: Connection,
    cooldown_ms: u64,
}

impl NotificationStore {
    /// Opens or creates the outbox under a private state directory.
    ///
    /// # Errors
    ///
    /// Returns an error for unsafe permissions, `SQLite` failure, or an unsupported future schema.
    pub fn open(directory: &Path) -> Result<Self, NotificationError> {
        let (_path, mut connection) = open_private_wal(directory, OsStr::new(DATABASE_NAME))?;
        migrate(&mut connection)?;
        Ok(Self {
            connection,
            cooldown_ms: policy::DEFAULT_COOLDOWN_MS,
        })
    }

    /// Persists one event for each unique destination not suppressed by cooldown.
    ///
    /// Recovery events always bypass cooldown. The return value is the number of newly queued
    /// deliveries.
    ///
    /// # Errors
    ///
    /// Returns an error for no destinations, duplicates, oversized fan-out, time overflow, or
    /// persistence failure.
    pub fn enqueue(
        &mut self,
        event: &NotificationEvent,
        destinations: &[DestinationId],
        enqueued_at_ms: u64,
    ) -> Result<usize, NotificationError> {
        validate_destinations(destinations)?;
        let retry_until_ms = enqueued_at_ms
            .checked_add(retry_lifetime(event.severity()))
            .ok_or(NotificationError::Configuration)?;
        let cooldown_start = enqueued_at_ms.saturating_sub(self.cooldown_ms);
        let transaction = self.connection.transaction()?;
        let eligible = eligible_destinations(&transaction, event, destinations, cooldown_start)?;
        if eligible.is_empty() {
            transaction.commit()?;
            return Ok(0);
        }
        codec::insert_event(&transaction, event, enqueued_at_ms)?;
        for destination in &eligible {
            transaction.execute(
                "INSERT INTO notification_delivery \
                 (event_id, destination_id, attempt_count, next_attempt_at_ms, retry_until_ms) \
                 VALUES (?1, ?2, 0, ?3, ?4)",
                params![
                    event.event_id().as_bytes().as_slice(),
                    destination.as_bytes().as_slice(),
                    to_i64(enqueued_at_ms)?,
                    to_i64(retry_until_ms)?,
                ],
            )?;
        }
        transaction.commit()?;
        Ok(eligible.len())
    }

    /// Returns a bounded oldest-first snapshot of pending due deliveries.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid limit, malformed durable data, or query failure.
    pub fn due(&self, now_ms: u64, limit: usize) -> Result<Vec<Delivery>, NotificationError> {
        if limit == 0 || limit > DELIVERY_BATCH_LIMIT {
            return Err(NotificationError::Configuration);
        }
        let mut statement = self.connection.prepare(
            "SELECT d.id, d.destination_id, d.attempt_count, \
                    e.event_id, e.occurred_at_ms, e.source, e.category, e.severity, \
                    e.account_local_id, e.reason_code, e.previous_state, e.current_state, \
                    e.human_summary, e.next_action, e.dedupe_key \
             FROM notification_delivery d \
             JOIN notification_event e ON e.event_id = d.event_id \
             WHERE d.delivered_at_ms IS NULL AND d.abandoned_at_ms IS NULL \
               AND d.next_attempt_at_ms <= ?1 \
             ORDER BY d.next_attempt_at_ms, d.id LIMIT ?2",
        )?;
        let limit = i64::try_from(limit).map_err(|_error| NotificationError::Configuration)?;
        let rows = statement.query_map(params![to_i64(now_ms)?, limit], decode_delivery)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// Marks one pending delivery successful.
    ///
    /// # Errors
    ///
    /// Returns an error when the delivery is unknown, already terminal, or persistence fails.
    pub fn mark_delivered(
        &mut self,
        delivery_id: DeliveryId,
        delivered_at_ms: u64,
    ) -> Result<(), NotificationError> {
        let updated = self.connection.execute(
            "UPDATE notification_delivery SET delivered_at_ms = ?1 \
             WHERE id = ?2 AND delivered_at_ms IS NULL AND abandoned_at_ms IS NULL",
            params![to_i64(delivered_at_ms)?, delivery_id.stored()],
        )?;
        if updated == 1 {
            Ok(())
        } else {
            Err(NotificationError::NotFound)
        }
    }

    /// Records one redacted adapter error code and schedules the next retry or abandonment.
    ///
    /// # Errors
    ///
    /// Returns an error for a zero code, unknown or terminal delivery, time overflow, malformed
    /// durable state, or persistence failure.
    pub fn record_failure(
        &mut self,
        delivery_id: DeliveryId,
        failed_at_ms: u64,
        error_code: u32,
    ) -> Result<FailureDisposition, NotificationError> {
        if error_code == 0 {
            return Err(NotificationError::Configuration);
        }
        let transaction = self.connection.transaction()?;
        let stored = transaction
            .query_row(
                "SELECT attempt_count, retry_until_ms FROM notification_delivery \
                 WHERE id = ?1 AND delivered_at_ms IS NULL AND abandoned_at_ms IS NULL",
                [delivery_id.stored()],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
            )
            .optional()?
            .ok_or(NotificationError::NotFound)?;
        let attempts =
            u32::try_from(stored.0).map_err(|_error| NotificationError::Configuration)?;
        let retry_until = codec::from_i64(stored.1)?;
        let next_attempt = failed_at_ms
            .checked_add(retry_delay(attempts)?)
            .ok_or(NotificationError::Configuration)?;
        let next_count = attempts
            .checked_add(1)
            .ok_or(NotificationError::Configuration)?;
        let disposition = if next_attempt > retry_until {
            transaction.execute(
                "UPDATE notification_delivery SET attempt_count = ?1, last_error_code = ?2, \
                 abandoned_at_ms = ?3 WHERE id = ?4",
                params![
                    i64::from(next_count),
                    i64::from(error_code),
                    to_i64(failed_at_ms)?,
                    delivery_id.stored(),
                ],
            )?;
            FailureDisposition::Abandoned
        } else {
            transaction.execute(
                "UPDATE notification_delivery SET attempt_count = ?1, last_error_code = ?2, \
                 next_attempt_at_ms = ?3 WHERE id = ?4",
                params![
                    i64::from(next_count),
                    i64::from(error_code),
                    to_i64(next_attempt)?,
                    delivery_id.stored(),
                ],
            )?;
            FailureDisposition::RetryAt(next_attempt)
        };
        transaction.commit()?;
        Ok(disposition)
    }
}

impl core::fmt::Debug for NotificationStore {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("NotificationStore")
            .field("cooldown_ms", &self.cooldown_ms)
            .finish_non_exhaustive()
    }
}
