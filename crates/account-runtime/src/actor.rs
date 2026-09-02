use std::path::PathBuf;
use std::thread;

use tokio::sync::{mpsc, oneshot};

use crate::{
    AccountLocalId, AccountRuntimeError, AccountSnapshot, AccountTransition, TransitionReceipt,
    store::AccountStore,
};

const MAX_QUEUE_CAPACITY: usize = 1_024;

/// Validated configuration for one independent account actor.
#[derive(Clone, Debug)]
pub struct AccountRuntimeConfig {
    state_directory: PathBuf,
    local_id: AccountLocalId,
    queue_capacity: usize,
}

impl AccountRuntimeConfig {
    /// Creates one account actor configuration.
    ///
    /// # Errors
    ///
    /// Returns an error for an empty state path or queue capacity outside the compiled bound.
    pub fn new(
        state_directory: PathBuf,
        local_id: AccountLocalId,
        queue_capacity: usize,
    ) -> Result<Self, AccountRuntimeError> {
        if state_directory.as_os_str().is_empty()
            || queue_capacity == 0
            || queue_capacity > MAX_QUEUE_CAPACITY
        {
            return Err(AccountRuntimeError::Configuration);
        }
        Ok(Self {
            state_directory,
            local_id,
            queue_capacity,
        })
    }

    /// Returns the installation-local identifier owned by this actor.
    #[must_use]
    pub const fn local_id(&self) -> AccountLocalId {
        self.local_id
    }

    /// Returns the configured bounded command capacity.
    #[must_use]
    pub const fn queue_capacity(&self) -> usize {
        self.queue_capacity
    }

    /// Returns the deterministic per-account database path.
    #[must_use]
    pub fn database_path(&self) -> PathBuf {
        self.local_id.database_path(&self.state_directory)
    }
}

/// Cloneable bounded command handle for one account writer.
#[derive(Clone)]
pub struct AccountHandle {
    local_id: AccountLocalId,
    sender: mpsc::Sender<Command>,
}

impl AccountHandle {
    /// Returns the installation-local account identifier.
    #[must_use]
    pub const fn local_id(&self) -> AccountLocalId {
        self.local_id
    }

    /// Returns currently unused slots in the bounded command queue.
    #[must_use]
    pub fn remaining_capacity(&self) -> usize {
        self.sender.capacity()
    }

    /// Reads the latest durable account state from the single writer.
    ///
    /// # Errors
    ///
    /// Returns an error when the actor stopped or persistence failed.
    pub async fn snapshot(&self) -> Result<AccountSnapshot, AccountRuntimeError> {
        let (reply, receive) = oneshot::channel();
        self.send(Command::Snapshot { reply }).await?;
        receive
            .await
            .map_err(|_error| AccountRuntimeError::Closed)?
    }

    /// Validates and atomically records one lifecycle transition.
    ///
    /// # Errors
    ///
    /// Returns an error when the actor stopped, the transition is invalid or persistence fails.
    pub async fn transition(
        &self,
        requested: AccountTransition,
    ) -> Result<TransitionReceipt, AccountRuntimeError> {
        let (reply, receive) = oneshot::channel();
        self.send(Command::Transition { requested, reply }).await?;
        receive
            .await
            .map_err(|_error| AccountRuntimeError::Closed)?
    }

    async fn send(&self, command: Command) -> Result<(), AccountRuntimeError> {
        self.sender
            .send(command)
            .await
            .map_err(|_error| AccountRuntimeError::Closed)
    }
}

impl core::fmt::Debug for AccountHandle {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("AccountHandle")
            .field("local_id", &self.local_id)
            .finish_non_exhaustive()
    }
}

/// Owned account actor and its writer-thread lifetime.
pub struct AccountRuntime {
    handle: AccountHandle,
    worker: Option<thread::JoinHandle<Result<(), AccountRuntimeError>>>,
}

impl AccountRuntime {
    /// Returns a cloneable bounded handle to the actor.
    #[must_use]
    pub fn handle(&self) -> AccountHandle {
        self.handle.clone()
    }

    /// Stops the writer after all earlier queued commands and joins its thread.
    ///
    /// # Errors
    ///
    /// Returns an error when the actor already stopped, failed persistence or its thread failed.
    pub async fn shutdown(mut self) -> Result<AccountSnapshot, AccountRuntimeError> {
        let (reply, receive) = oneshot::channel();
        self.handle.send(Command::Shutdown { reply }).await?;
        let snapshot = receive
            .await
            .map_err(|_error| AccountRuntimeError::Closed)??;
        let worker = self
            .worker
            .take()
            .ok_or(AccountRuntimeError::WorkerFailed)?;
        tokio::task::spawn_blocking(move || worker.join())
            .await
            .map_err(|_error| AccountRuntimeError::WorkerFailed)?
            .map_err(|_panic| AccountRuntimeError::WorkerFailed)??;
        Ok(snapshot)
    }
}

impl core::fmt::Debug for AccountRuntime {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("AccountRuntime")
            .field("handle", &self.handle)
            .finish_non_exhaustive()
    }
}

/// Opens durable state and starts one dedicated account writer.
///
/// # Errors
///
/// Returns an error for invalid storage, failed crash recovery or thread creation.
pub async fn spawn_account(
    config: AccountRuntimeConfig,
    recovery_at_ms: u64,
) -> Result<AccountRuntime, AccountRuntimeError> {
    let directory = config.state_directory.clone();
    let local_id = config.local_id;
    let store = tokio::task::spawn_blocking(move || {
        AccountStore::open(&directory, local_id, recovery_at_ms)
    })
    .await
    .map_err(|_error| AccountRuntimeError::WorkerFailed)??;
    let (sender, receiver) = mpsc::channel(config.queue_capacity);
    let worker = thread::Builder::new()
        .name(format!("lirvena-account-{}", local_id.file_stem()))
        .spawn(move || run_writer(store, receiver))
        .map_err(|_error| AccountRuntimeError::WorkerFailed)?;
    Ok(AccountRuntime {
        handle: AccountHandle { local_id, sender },
        worker: Some(worker),
    })
}

enum Command {
    Snapshot {
        reply: oneshot::Sender<Result<AccountSnapshot, AccountRuntimeError>>,
    },
    Transition {
        requested: AccountTransition,
        reply: oneshot::Sender<Result<TransitionReceipt, AccountRuntimeError>>,
    },
    Shutdown {
        reply: oneshot::Sender<Result<AccountSnapshot, AccountRuntimeError>>,
    },
}

fn run_writer(
    mut store: AccountStore,
    mut receiver: mpsc::Receiver<Command>,
) -> Result<(), AccountRuntimeError> {
    while let Some(command) = receiver.blocking_recv() {
        match command {
            Command::Snapshot { reply } => {
                let _ignored = reply.send(store.snapshot());
            }
            Command::Transition { requested, reply } => {
                let _ignored = reply.send(store.transition(requested));
            }
            Command::Shutdown { reply } => {
                let snapshot = store.snapshot();
                let failed = snapshot.is_err();
                let _ignored = reply.send(snapshot);
                return if failed {
                    Err(AccountRuntimeError::Persistence)
                } else {
                    Ok(())
                };
            }
        }
    }
    Ok(())
}
