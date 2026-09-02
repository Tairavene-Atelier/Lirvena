use core::time::Duration;

use ceylith_protocol::{MAX_WATCH_WAIT_MS, WatchEvent, WatchOutcome};
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;
use tokio::time::sleep;

use super::InstallationClientRuntime;
use crate::ClientError;

const IDLE_BACKOFF: Duration = Duration::from_secs(1);
const MAX_EVENT_QUEUE_CAPACITY: usize = 64;

/// Receiver for typed Ceylith continuity events from one dedicated connection.
pub struct InstallationWatch {
    receiver: mpsc::Receiver<Result<WatchEvent, ClientError>>,
}

impl InstallationWatch {
    /// Waits for the next continuity event or terminal failure.
    pub async fn next(&mut self) -> Option<Result<WatchEvent, ClientError>> {
        self.receiver.recv().await
    }
}

impl core::fmt::Debug for InstallationWatch {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("InstallationWatch")
            .field("remaining_capacity", &self.receiver.capacity())
            .finish_non_exhaustive()
    }
}

/// Owned lifetime of a dedicated Ceylith continuity Watch connection.
pub struct InstallationWatchRuntime {
    watch: InstallationWatch,
    shutdown: Option<oneshot::Sender<()>>,
    task: Option<JoinHandle<()>>,
}

impl InstallationWatchRuntime {
    /// Splits the event receiver from its owned task lifetime.
    pub fn watch(&mut self) -> &mut InstallationWatch {
        &mut self.watch
    }

    /// Stops the Watch task and closes its dedicated connection.
    ///
    /// # Errors
    ///
    /// Returns `Worker` if the Watch task failed to join.
    pub async fn shutdown(mut self) -> Result<(), ClientError> {
        if let Some(shutdown) = self.shutdown.take() {
            let _ignored = shutdown.send(());
        }
        let task = self.task.take().ok_or(ClientError::Worker)?;
        task.await.map_err(|_| ClientError::Worker)
    }
}

impl Drop for InstallationWatchRuntime {
    fn drop(&mut self) {
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

impl core::fmt::Debug for InstallationWatchRuntime {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("InstallationWatchRuntime")
            .field("watch", &self.watch)
            .finish_non_exhaustive()
    }
}

/// Starts repeated cursor-bound Watch calls over a dedicated installation connection.
///
/// Immediate idle responses are rate-limited locally. This keeps compatibility with an early
/// Ceylith that has not yet implemented held long polling without creating a busy loop.
///
/// # Errors
///
/// Returns an error for a zero or oversized event queue.
pub fn spawn_installation_watch(
    installation: InstallationClientRuntime,
    event_queue_capacity: usize,
) -> Result<InstallationWatchRuntime, ClientError> {
    if !(1..=MAX_EVENT_QUEUE_CAPACITY).contains(&event_queue_capacity) {
        installation.abort();
        return Err(ClientError::Configuration);
    }
    let (events, receiver) = mpsc::channel(event_queue_capacity);
    let (shutdown, stop) = oneshot::channel();
    let runtime = match tokio::runtime::Handle::try_current() {
        Ok(runtime) => runtime,
        Err(_error) => {
            installation.abort();
            return Err(ClientError::Configuration);
        }
    };
    let task = runtime.spawn(run(installation, events, stop));
    Ok(InstallationWatchRuntime {
        watch: InstallationWatch { receiver },
        shutdown: Some(shutdown),
        task: Some(task),
    })
}

async fn run(
    installation: InstallationClientRuntime,
    events: mpsc::Sender<Result<WatchEvent, ClientError>>,
    mut stop: oneshot::Receiver<()>,
) {
    let client = installation.client();
    let mut cursor = 0_u64;
    loop {
        let outcome = tokio::select! {
            _ = &mut stop => {
                installation.abort();
                return;
            }
            outcome = client.watch_once(cursor, MAX_WATCH_WAIT_MS) => outcome,
        };
        match outcome {
            Ok(WatchOutcome::Event(event)) => {
                cursor = event.cursor();
                if events.send(Ok(event)).await.is_err() {
                    installation.abort();
                    return;
                }
            }
            Ok(WatchOutcome::Idle { .. }) => {
                tokio::select! {
                    _ = &mut stop => {
                        installation.abort();
                        return;
                    }
                    () = sleep(IDLE_BACKOFF) => {}
                }
            }
            Err(error) => {
                let _ignored = events.send(Err(error)).await;
                installation.abort();
                return;
            }
        }
    }
}
