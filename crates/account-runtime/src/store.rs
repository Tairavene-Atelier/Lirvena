use std::path::Path;

use local_state::open_private_wal;
use rusqlite::{Connection, OptionalExtension, params};

use crate::{
    AccountLocalId, AccountPhase, AccountRuntimeError, AccountSnapshot, AccountTransition,
    ProtectiveReason, TransitionReceipt,
};

mod schema;

use schema::migrate;

pub(crate) struct AccountStore {
    local_id: AccountLocalId,
    connection: Connection,
}

impl AccountStore {
    pub(crate) fn open(
        directory: &Path,
        local_id: AccountLocalId,
        recovery_at_ms: u64,
    ) -> Result<Self, AccountRuntimeError> {
        let path = local_id.database_path(directory);
        let file_name = path.file_name().ok_or(AccountRuntimeError::Configuration)?;
        let (_path, mut connection) = open_private_wal(directory, file_name)?;
        migrate(&mut connection, recovery_at_ms)?;
        let mut store = Self {
            local_id,
            connection,
        };
        store.recover_interrupted(recovery_at_ms)?;
        Ok(store)
    }

    pub(crate) fn snapshot(&self) -> Result<AccountSnapshot, AccountRuntimeError> {
        let stored = self
            .connection
            .query_row(
                "SELECT phase, generation, protective_reason, updated_at_ms \
                 FROM account_state WHERE singleton = 1",
                [],
                |row| {
                    Ok((
                        row.get::<_, u8>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, Option<u8>>(2)?,
                        row.get::<_, i64>(3)?,
                    ))
                },
            )
            .optional()?
            .ok_or(AccountRuntimeError::Persistence)?;
        let phase =
            AccountPhase::from_stored(stored.0).ok_or(AccountRuntimeError::Configuration)?;
        let generation =
            u64::try_from(stored.1).map_err(|_error| AccountRuntimeError::Configuration)?;
        let reason = match stored.2 {
            Some(value) => Some(
                ProtectiveReason::from_stored(value).ok_or(AccountRuntimeError::Configuration)?,
            ),
            None => None,
        };
        let updated_at_ms =
            u64::try_from(stored.3).map_err(|_error| AccountRuntimeError::Configuration)?;
        validate_reason(phase, reason)?;
        Ok(AccountSnapshot::new(
            self.local_id,
            phase,
            generation,
            reason,
            updated_at_ms,
        ))
    }

    pub(crate) fn transition(
        &mut self,
        requested: AccountTransition,
    ) -> Result<TransitionReceipt, AccountRuntimeError> {
        let previous = self.snapshot()?;
        validate_transition(previous, requested)?;
        let generation = if requested.next == AccountPhase::Starting {
            previous
                .generation()
                .checked_add(1)
                .ok_or(AccountRuntimeError::TransitionRejected)?
        } else {
            previous.generation()
        };
        let current = AccountSnapshot::new(
            self.local_id,
            requested.next,
            generation,
            requested.protective_reason,
            requested.occurred_at_ms,
        );
        self.commit_transition(previous, current)?;
        Ok(TransitionReceipt::new(previous, current))
    }

    fn recover_interrupted(&mut self, occurred_at_ms: u64) -> Result<(), AccountRuntimeError> {
        let snapshot = self.snapshot()?;
        if matches!(
            snapshot.phase(),
            AccountPhase::Starting | AccountPhase::Active
        ) {
            self.transition(AccountTransition {
                next: AccountPhase::ProtectiveOffline,
                protective_reason: Some(ProtectiveReason::ProcessRestart),
                occurred_at_ms: occurred_at_ms.max(snapshot.updated_at_ms()),
            })?;
        }
        Ok(())
    }

    fn commit_transition(
        &mut self,
        previous: AccountSnapshot,
        current: AccountSnapshot,
    ) -> Result<(), AccountRuntimeError> {
        let transaction = self.connection.transaction()?;
        let updated = transaction.execute(
            "UPDATE account_state SET phase = ?1, generation = ?2, protective_reason = ?3, \
             updated_at_ms = ?4 WHERE singleton = 1 AND phase = ?5 AND generation = ?6",
            params![
                current.phase() as u8,
                to_i64(current.generation())?,
                current.protective_reason().map(|reason| reason as u8),
                to_i64(current.updated_at_ms())?,
                previous.phase() as u8,
                to_i64(previous.generation())?,
            ],
        )?;
        if updated != 1 {
            return Err(AccountRuntimeError::Persistence);
        }
        transaction.execute(
            "INSERT INTO account_transitions \
             (generation, from_phase, to_phase, protective_reason, occurred_at_ms) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                to_i64(current.generation())?,
                previous.phase() as u8,
                current.phase() as u8,
                current.protective_reason().map(|reason| reason as u8),
                to_i64(current.updated_at_ms())?,
            ],
        )?;
        transaction.commit().map_err(Into::into)
    }
}

fn validate_transition(
    previous: AccountSnapshot,
    requested: AccountTransition,
) -> Result<(), AccountRuntimeError> {
    validate_reason(requested.next, requested.protective_reason)
        .map_err(|_error| AccountRuntimeError::TransitionRejected)?;
    if requested.occurred_at_ms < previous.updated_at_ms() {
        return Err(AccountRuntimeError::TransitionRejected);
    }
    let allowed = matches!(
        (previous.phase(), requested.next),
        (
            AccountPhase::Stopped | AccountPhase::ProtectiveOffline,
            AccountPhase::Starting
        ) | (
            AccountPhase::Starting,
            AccountPhase::Active | AccountPhase::Stopped | AccountPhase::ProtectiveOffline
        ) | (
            AccountPhase::Active,
            AccountPhase::Stopped | AccountPhase::ProtectiveOffline
        )
    );
    if allowed {
        Ok(())
    } else {
        Err(AccountRuntimeError::TransitionRejected)
    }
}

fn validate_reason(
    phase: AccountPhase,
    reason: Option<ProtectiveReason>,
) -> Result<(), AccountRuntimeError> {
    if (phase == AccountPhase::ProtectiveOffline) == reason.is_some() {
        Ok(())
    } else {
        Err(AccountRuntimeError::Configuration)
    }
}

fn to_i64(value: u64) -> Result<i64, AccountRuntimeError> {
    i64::try_from(value).map_err(|_error| AccountRuntimeError::Configuration)
}
