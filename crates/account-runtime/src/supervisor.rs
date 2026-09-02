use std::collections::BTreeMap;

use crate::{
    AccountHandle, AccountLocalId, AccountRuntime, AccountRuntimeConfig, AccountRuntimeError,
    AccountSnapshot, spawn_account,
};

/// Installation-local owner of independent account actor lifetimes.
#[derive(Debug, Default)]
pub struct AccountSupervisor {
    accounts: BTreeMap<AccountLocalId, AccountRuntime>,
}

impl AccountSupervisor {
    /// Creates an empty account supervisor.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            accounts: BTreeMap::new(),
        }
    }

    /// Returns the number of independently owned account actors.
    #[must_use]
    pub fn len(&self) -> usize {
        self.accounts.len()
    }

    /// Returns whether no account actors are owned.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.accounts.is_empty()
    }

    /// Opens and inserts one account actor.
    ///
    /// # Errors
    ///
    /// Returns an error for a duplicate local identifier or failed actor startup.
    pub async fn spawn(
        &mut self,
        config: AccountRuntimeConfig,
        recovery_at_ms: u64,
    ) -> Result<AccountHandle, AccountRuntimeError> {
        let local_id = config.local_id();
        if self.accounts.contains_key(&local_id) {
            return Err(AccountRuntimeError::DuplicateAccount);
        }
        let runtime = spawn_account(config, recovery_at_ms).await?;
        let handle = runtime.handle();
        self.accounts.insert(local_id, runtime);
        Ok(handle)
    }

    /// Returns a bounded handle for an owned account.
    ///
    /// # Errors
    ///
    /// Returns an error when the local identifier is unknown.
    pub fn handle(&self, local_id: AccountLocalId) -> Result<AccountHandle, AccountRuntimeError> {
        self.accounts
            .get(&local_id)
            .map(AccountRuntime::handle)
            .ok_or(AccountRuntimeError::UnknownAccount)
    }

    /// Stops and removes one account actor.
    ///
    /// # Errors
    ///
    /// Returns an error when the account is unknown or shutdown fails.
    pub async fn shutdown(
        &mut self,
        local_id: AccountLocalId,
    ) -> Result<AccountSnapshot, AccountRuntimeError> {
        let runtime = self
            .accounts
            .remove(&local_id)
            .ok_or(AccountRuntimeError::UnknownAccount)?;
        runtime.shutdown().await
    }

    /// Stops every owned account, continuing after individual failures.
    ///
    /// # Errors
    ///
    /// Returns the first shutdown error after all actors have been attempted.
    pub async fn shutdown_all(&mut self) -> Result<(), AccountRuntimeError> {
        let accounts = core::mem::take(&mut self.accounts);
        let mut first_error = None;
        for (_local_id, runtime) in accounts {
            if let Err(error) = runtime.shutdown().await {
                first_error.get_or_insert(error);
            }
        }
        first_error.map_or(Ok(()), Err)
    }
}
