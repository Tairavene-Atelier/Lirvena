use rusqlite::{Transaction, params};

use super::Delivery;
use crate::{
    DedupeKey, DeliveryId, DestinationId, EventCategory, EventId, EventSource, EventState,
    NotificationError, NotificationEvent, NotificationText, Severity, StateTransition,
};

pub(super) fn insert_event(
    transaction: &Transaction<'_>,
    event: &NotificationEvent,
    enqueued_at_ms: u64,
) -> Result<(), NotificationError> {
    transaction.execute(
        "INSERT INTO notification_event \
         (event_id, occurred_at_ms, enqueued_at_ms, source, category, severity, account_local_id, \
          reason_code, previous_state, current_state, human_summary, next_action, dedupe_key) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
        params![
            event.event_id().as_bytes().as_slice(),
            to_i64(event.occurred_at_ms())?,
            to_i64(enqueued_at_ms)?,
            event.source() as u8,
            event.category() as u8,
            event.severity() as u8,
            event.account_local_id().map(<[u8; 16]>::as_slice),
            event.reason_code(),
            event.transition().previous() as u8,
            event.transition().current() as u8,
            event.human_summary().as_str(),
            event.next_action().as_str(),
            event.dedupe_key().as_bytes().as_slice(),
        ],
    )?;
    Ok(())
}

pub(super) fn decode_delivery(row: &rusqlite::Row<'_>) -> Result<Delivery, rusqlite::Error> {
    decode_delivery_inner(row).map_err(|_error| rusqlite::Error::InvalidQuery)
}

fn decode_delivery_inner(row: &rusqlite::Row<'_>) -> Result<Delivery, NotificationError> {
    let id = DeliveryId::from_stored(row.get(0)?)?;
    let destination_id = DestinationId::from_slice(&row.get::<_, Vec<u8>>(1)?)?;
    let attempt_count =
        u32::try_from(row.get::<_, i64>(2)?).map_err(|_error| NotificationError::Configuration)?;
    let event_id = EventId::from_slice(&row.get::<_, Vec<u8>>(3)?)?;
    let occurred_at_ms = from_i64(row.get(4)?)?;
    let source = EventSource::from_stored(row.get(5)?)?;
    let category = EventCategory::from_stored(row.get(6)?)?;
    let severity = Severity::from_stored(row.get(7)?)?;
    let account_local_id = row
        .get::<_, Option<Vec<u8>>>(8)?
        .map(|bytes| {
            bytes
                .try_into()
                .map_err(|_error| NotificationError::Configuration)
        })
        .transpose()?;
    let reason_code =
        u32::try_from(row.get::<_, i64>(9)?).map_err(|_error| NotificationError::Configuration)?;
    let transition = StateTransition::new(
        EventState::from_stored(row.get(10)?)?,
        EventState::from_stored(row.get(11)?)?,
    )?;
    let event = NotificationEvent::new(
        event_id,
        occurred_at_ms,
        source,
        category,
        severity,
        account_local_id,
        reason_code,
        transition,
        NotificationText::new(row.get::<_, String>(12)?)?,
        NotificationText::new(row.get::<_, String>(13)?)?,
        DedupeKey::from_slice(&row.get::<_, Vec<u8>>(14)?)?,
    )?;
    Ok(Delivery {
        id,
        destination_id,
        attempt_count,
        event,
    })
}

pub(super) fn to_i64(value: u64) -> Result<i64, NotificationError> {
    i64::try_from(value).map_err(|_error| NotificationError::Configuration)
}

pub(super) fn from_i64(value: i64) -> Result<u64, NotificationError> {
    u64::try_from(value).map_err(|_error| NotificationError::Configuration)
}
