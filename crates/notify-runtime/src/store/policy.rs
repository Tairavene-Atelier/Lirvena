use std::collections::BTreeSet;

use rusqlite::{OptionalExtension, Transaction, params};

use crate::{DestinationId, EventCategory, NotificationError, NotificationEvent, Severity};

pub(super) const DEFAULT_COOLDOWN_MS: u64 = 15 * 60 * 1_000;
const MAX_DESTINATIONS: usize = 32;
const RETRY_DELAYS_MS: [u64; 4] = [60_000, 5 * 60_000, 30 * 60_000, 2 * 60 * 60_000];

pub(super) fn validate_destinations(
    destinations: &[DestinationId],
) -> Result<(), NotificationError> {
    if destinations.is_empty() || destinations.len() > MAX_DESTINATIONS {
        return Err(NotificationError::Configuration);
    }
    let unique = destinations.iter().copied().collect::<BTreeSet<_>>();
    if unique.len() == destinations.len() {
        Ok(())
    } else {
        Err(NotificationError::Configuration)
    }
}

pub(super) fn eligible_destinations(
    transaction: &Transaction<'_>,
    event: &NotificationEvent,
    destinations: &[DestinationId],
    cooldown_start: u64,
) -> Result<Vec<DestinationId>, NotificationError> {
    if event.category() == EventCategory::Recovery {
        return Ok(destinations.to_vec());
    }
    let mut eligible = Vec::with_capacity(destinations.len());
    let mut statement = transaction.prepare(
        "SELECT 1 FROM notification_delivery d \
         JOIN notification_event e ON e.event_id = d.event_id \
         WHERE d.destination_id = ?1 AND e.dedupe_key = ?2 AND e.enqueued_at_ms >= ?3 \
         LIMIT 1",
    )?;
    for destination in destinations {
        let duplicate = statement
            .query_row(
                params![
                    destination.as_bytes().as_slice(),
                    event.dedupe_key().as_bytes().as_slice(),
                    super::codec::to_i64(cooldown_start)?,
                ],
                |_row| Ok(()),
            )
            .optional()?
            .is_some();
        if !duplicate {
            eligible.push(*destination);
        }
    }
    Ok(eligible)
}

pub(super) fn retry_delay(attempts: u32) -> Result<u64, NotificationError> {
    let index = usize::try_from(attempts)
        .map_err(|_error| NotificationError::Configuration)?
        .min(RETRY_DELAYS_MS.len() - 1);
    Ok(RETRY_DELAYS_MS[index])
}

pub(super) const fn retry_lifetime(severity: Severity) -> u64 {
    match severity {
        Severity::Info => 30 * 60 * 1_000,
        Severity::Warning => 2 * 60 * 60 * 1_000,
        Severity::Critical => 24 * 60 * 60 * 1_000,
    }
}
