use core::time::Duration;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;
use tokio::time::interval;

use crate::{
    DestinationId, NotificationAdapter, NotificationError, NotificationEvent, NotificationStore,
};

const MAX_QUEUE_CAPACITY: usize = 1_024;
const DELIVERY_BATCH: usize = 100;
const SWEEP_INTERVAL: Duration = Duration::from_secs(30);

/// Validated notification worker configuration.
pub struct NotificationRuntimeConfig {
    state_directory: PathBuf,
    adapters: Vec<NotificationAdapter>,
    queue_capacity: usize,
}

impl NotificationRuntimeConfig {
    /// Creates one outbox worker configuration.
    ///
    /// # Errors
    ///
    /// Returns an error for no adapters, duplicate destinations, or an invalid queue capacity.
    pub fn new(
        state_directory: PathBuf,
        adapters: Vec<NotificationAdapter>,
        queue_capacity: usize,
    ) -> Result<Self, NotificationError> {
        if state_directory.as_os_str().is_empty()
            || adapters.is_empty()
            || queue_capacity == 0
            || queue_capacity > MAX_QUEUE_CAPACITY
        {
            return Err(NotificationError::Configuration);
        }
        let mut destinations = BTreeSet::new();
        for adapter in &adapters {
            if !destinations.insert(adapter.destination_id()) {
                return Err(NotificationError::Configuration);
            }
        }
        Ok(Self {
            state_directory,
            adapters,
            queue_capacity,
        })
    }
}

impl core::fmt::Debug for NotificationRuntimeConfig {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("NotificationRuntimeConfig")
            .field("state_directory", &self.state_directory)
            .field("adapter_count", &self.adapters.len())
            .field("queue_capacity", &self.queue_capacity)
            .finish()
    }
}

/// Cloneable bounded handle that never performs network delivery on the caller task.
#[derive(Clone)]
pub struct NotificationHandle {
    sender: mpsc::Sender<Command>,
}

impl NotificationHandle {
    /// Persists an event for every configured destination.
    ///
    /// # Errors
    ///
    /// Returns an error when the worker stopped or persistence rejected the event.
    pub async fn enqueue(
        &self,
        event: NotificationEvent,
        enqueued_at_ms: u64,
    ) -> Result<usize, NotificationError> {
        let (reply, response) = oneshot::channel();
        self.sender
            .send(Command::Enqueue {
                event,
                enqueued_at_ms,
                reply: Some(reply),
            })
            .await
            .map_err(|_error| NotificationError::Closed)?;
        response.await.map_err(|_error| NotificationError::Closed)?
    }

    /// Offers an event to the bounded persistence queue without waiting.
    ///
    /// This is the QQ safety-state path: it never waits for storage or notification network I/O.
    ///
    /// # Errors
    ///
    /// Returns `Busy` when the queue is full or `Closed` when the worker stopped.
    pub fn try_enqueue(
        &self,
        event: NotificationEvent,
        enqueued_at_ms: u64,
    ) -> Result<(), NotificationError> {
        self.sender
            .try_send(Command::Enqueue {
                event,
                enqueued_at_ms,
                reply: None,
            })
            .map_err(|error| match error {
                mpsc::error::TrySendError::Full(_command) => NotificationError::Busy,
                mpsc::error::TrySendError::Closed(_command) => NotificationError::Closed,
            })
    }

    /// Requests one immediate bounded due-delivery sweep.
    ///
    /// # Errors
    ///
    /// Returns an error when the worker stopped or outbox persistence failed.
    pub async fn flush(&self, now_ms: u64) -> Result<DeliverySweep, NotificationError> {
        let (reply, response) = oneshot::channel();
        self.sender
            .send(Command::Flush { now_ms, reply })
            .await
            .map_err(|_error| NotificationError::Closed)?;
        response.await.map_err(|_error| NotificationError::Closed)?
    }
}

/// Summary of one bounded outbox delivery sweep.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DeliverySweep {
    attempted: usize,
    delivered: usize,
    failed: usize,
}

impl DeliverySweep {
    /// Returns the number of due deliveries attempted.
    #[must_use]
    pub const fn attempted(self) -> usize {
        self.attempted
    }

    /// Returns the number accepted by remote destinations.
    #[must_use]
    pub const fn delivered(self) -> usize {
        self.delivered
    }

    /// Returns the number retained or abandoned after a failed attempt.
    #[must_use]
    pub const fn failed(self) -> usize {
        self.failed
    }
}

impl core::fmt::Debug for NotificationHandle {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("NotificationHandle")
            .field("remaining_capacity", &self.sender.capacity())
            .finish_non_exhaustive()
    }
}

/// Owned notification worker lifetime.
pub struct NotificationRuntime {
    handle: NotificationHandle,
    task: Option<JoinHandle<()>>,
}

impl NotificationRuntime {
    /// Returns a cloneable bounded enqueue handle.
    #[must_use]
    pub fn handle(&self) -> NotificationHandle {
        self.handle.clone()
    }

    /// Stops the worker after all previously queued commands.
    ///
    /// # Errors
    ///
    /// Returns an error when the worker already stopped or its task failed.
    pub async fn shutdown(mut self) -> Result<(), NotificationError> {
        let (reply, response) = oneshot::channel();
        self.handle
            .sender
            .send(Command::Shutdown { reply })
            .await
            .map_err(|_error| NotificationError::Closed)?;
        response.await.map_err(|_error| NotificationError::Closed)?;
        let task = self.task.take().ok_or(NotificationError::Worker)?;
        task.await.map_err(|_error| NotificationError::Worker)
    }
}

impl Drop for NotificationRuntime {
    fn drop(&mut self) {
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

impl core::fmt::Debug for NotificationRuntime {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("NotificationRuntime")
            .field("handle", &self.handle)
            .finish_non_exhaustive()
    }
}

/// Opens the private outbox and starts one bounded delivery worker.
///
/// # Errors
///
/// Returns an error for persistence failure or when called outside a Tokio runtime.
pub async fn spawn_notification_runtime(
    config: NotificationRuntimeConfig,
) -> Result<NotificationRuntime, NotificationError> {
    let state_directory = config.state_directory.clone();
    let store = tokio::task::spawn_blocking(move || NotificationStore::open(&state_directory))
        .await
        .map_err(|_error| NotificationError::Worker)??;
    let adapters = config
        .adapters
        .into_iter()
        .map(|adapter| (adapter.destination_id(), adapter))
        .collect::<BTreeMap<_, _>>();
    let destinations = adapters.keys().copied().collect::<Vec<_>>();
    let (sender, receiver) = mpsc::channel(config.queue_capacity);
    let handle = NotificationHandle { sender };
    let runtime =
        tokio::runtime::Handle::try_current().map_err(|_error| NotificationError::Configuration)?;
    let task = runtime.spawn(run(store, adapters, destinations, receiver));
    Ok(NotificationRuntime {
        handle,
        task: Some(task),
    })
}

enum Command {
    Enqueue {
        event: NotificationEvent,
        enqueued_at_ms: u64,
        reply: Option<oneshot::Sender<Result<usize, NotificationError>>>,
    },
    Flush {
        now_ms: u64,
        reply: oneshot::Sender<Result<DeliverySweep, NotificationError>>,
    },
    Shutdown {
        reply: oneshot::Sender<()>,
    },
}

async fn run(
    mut store: NotificationStore,
    adapters: BTreeMap<DestinationId, NotificationAdapter>,
    destinations: Vec<DestinationId>,
    mut receiver: mpsc::Receiver<Command>,
) {
    let mut sweep = interval(SWEEP_INTERVAL);
    sweep.reset();
    loop {
        tokio::select! {
            command = receiver.recv() => match command {
                Some(Command::Enqueue { event, enqueued_at_ms, reply }) => {
                    let result = store.enqueue(&event, &destinations, enqueued_at_ms);
                    if let Some(reply) = reply {
                        let _ignored = reply.send(result);
                    } else if result.is_err() {
                        return;
                    }
                }
                Some(Command::Flush { now_ms, reply }) => {
                    let result = deliver_due(&mut store, &adapters, now_ms).await;
                    let terminal = result.is_err();
                    let _ignored = reply.send(result);
                    if terminal {
                        return;
                    }
                }
                Some(Command::Shutdown { reply }) => {
                    if let Ok(now_ms) = now_ms() {
                        let _ignored = deliver_due(&mut store, &adapters, now_ms).await;
                    }
                    let _ignored = reply.send(());
                    return;
                }
                None => return,
            },
            _ = sweep.tick() => {
                let Ok(now_ms) = now_ms() else {
                    return;
                };
                if deliver_due(&mut store, &adapters, now_ms).await.is_err() {
                    return;
                }
            }
        }
    }
}

async fn deliver_due(
    store: &mut NotificationStore,
    adapters: &BTreeMap<DestinationId, NotificationAdapter>,
    now_ms: u64,
) -> Result<DeliverySweep, NotificationError> {
    let due = store.due(now_ms, DELIVERY_BATCH)?;
    let mut sweep = DeliverySweep {
        attempted: due.len(),
        ..DeliverySweep::default()
    };
    for delivery in due {
        let outcome = match adapters.get(&delivery.destination_id()) {
            Some(adapter) => adapter.deliver(&delivery, now_ms).await,
            None => Err(crate::AdapterError::Configuration),
        };
        match outcome {
            Ok(()) => {
                store.mark_delivered(delivery.id(), now_ms)?;
                sweep.delivered += 1;
            }
            Err(error) => {
                let _disposition = store.record_failure(delivery.id(), now_ms, error.code())?;
                sweep.failed += 1;
            }
        }
    }
    Ok(sweep)
}

fn now_ms() -> Result<u64, NotificationError> {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_error| NotificationError::Configuration)?;
    u64::try_from(elapsed.as_millis()).map_err(|_error| NotificationError::Configuration)
}
