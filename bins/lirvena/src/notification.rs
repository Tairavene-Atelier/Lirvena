mod config;
mod event;

use std::io;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(target_os = "linux")]
use account_runtime::{AccountLocalId, ProtectiveReason};
#[cfg(target_os = "linux")]
use ceylith_protocol::WatchEvent;
use notify_runtime::spawn_notification_runtime;
#[cfg(target_os = "linux")]
use notify_runtime::{NotificationEvent, NotificationHandle, NotificationRuntime};

pub(super) use config::AdapterSelection;

#[cfg(target_os = "linux")]
pub(super) struct NotificationCenter {
    runtime: NotificationRuntime,
}

#[cfg(target_os = "linux")]
impl NotificationCenter {
    pub(super) fn handle(&self) -> NotificationHandle {
        self.runtime.handle()
    }

    pub(super) async fn shutdown(self) -> Result<(), io::Error> {
        self.runtime
            .shutdown()
            .await
            .map_err(|_| io::Error::other("notification runtime shutdown failed"))
    }
}

#[cfg(target_os = "linux")]
pub(super) async fn start(state_directory: &Path) -> Result<Option<NotificationCenter>, io::Error> {
    let Some(prepared) = config::from_environment(state_directory, AdapterSelection::All, true)?
    else {
        return Ok(None);
    };
    let runtime = spawn_notification_runtime(prepared.runtime)
        .await
        .map_err(|_| io::Error::other("notification runtime startup failed"))?;
    Ok(Some(NotificationCenter { runtime }))
}

pub(super) async fn test(
    state_directory: &Path,
    selection: AdapterSelection,
) -> Result<(), Box<dyn std::error::Error>> {
    let prepared = config::from_environment(state_directory, selection, false)?
        .ok_or_else(|| io::Error::other("notification configuration is unavailable"))?;
    let expected = prepared.adapter_count;
    let runtime = spawn_notification_runtime(prepared.runtime).await?;
    let handle = runtime.handle();
    let now = now_ms()?;
    let queued = handle.enqueue(event::test_event(now)?, now).await?;
    let sweep = handle.flush(now).await?;
    runtime.shutdown().await?;
    if queued != expected || sweep.delivered() != expected || sweep.failed() != 0 {
        return Err(io::Error::other("one or more notification test deliveries failed").into());
    }
    println!("Lirvena notification test delivered to {expected} adapter(s)");
    Ok(())
}

#[cfg(target_os = "linux")]
pub(super) fn watch_event(
    event: &WatchEvent,
    account: AccountLocalId,
) -> Result<NotificationEvent, io::Error> {
    self::event::from_watch(event, account)
}

#[cfg(target_os = "linux")]
pub(super) fn protective_offline_event(
    occurred_at_ms: u64,
    account: AccountLocalId,
    reason: ProtectiveReason,
) -> Result<NotificationEvent, io::Error> {
    self::event::protective_offline(occurred_at_ms, account, reason)
}

fn now_ms() -> Result<u64, io::Error> {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| io::Error::other("system clock precedes Unix epoch"))?;
    u64::try_from(elapsed.as_millis()).map_err(|_| io::Error::other("system clock overflow"))
}
