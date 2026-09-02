mod ceylith;
mod continuity;
mod credential;
mod flow;
mod polling;
mod qq;

use account_runtime::{
    AccountGrantRequest, AccountLocalId, AccountPhase, AccountRuntimeConfig, AccountTransition,
    AssignedRealm, GrantAvailability, ProtectiveReason, plan_account_grants, spawn_account,
};
use ceylith_client::{RequestedAccess, spawn_installation_watch};
use ceylith_protocol::{GrantClass, SessionAdmission};
use notify_runtime::NotificationHandle;

use self::ceylith::{connect, ensure_matching_admission, negotiate_profile, runtime};
use self::continuity::{ContinuityAction, classify};
use crate::config::ProcessConfig;
use crate::notification;
use crate::support::now_ms;

pub(super) async fn run(config: ProcessConfig) -> Result<(), Box<dyn std::error::Error>> {
    let account_events = AccountEventHub::new(1_024)?;
    let event_publisher = account_events.publisher();
    let notification_center = notification::start(&config.state_directory).await?;
    let notification_handle = notification_center
        .as_ref()
        .map(notification::NotificationCenter::handle);
    let client_runtime = runtime()?;
    let ceylith_runtime = connect(&config, &client_runtime).await?;
    let ceylith = ceylith_runtime.client();
    eprintln!(
        "Lirvena connected to Ceylith with {:?} grant",
        ceylith.admission().grant_class()
    );
    let account_local_id = AccountLocalId::from_bytes(config.account_slot_id);
    let grant_plan = plan_account_grants(
        [AccountGrantRequest::new(
            account_local_id,
            config.account_mode,
        )],
        grant_availability(ceylith.admission()),
    )?;
    if grant_plan.public_fallbacks().contains(&account_local_id) {
        eprintln!(
            "WARNING: Lirvena Full grant is unavailable; this account is starting in Public mode"
        );
    }
    let requested_access = match grant_plan.assigned_realm(account_local_id) {
        Some(AssignedRealm::Public) => RequestedAccess::Public,
        Some(AssignedRealm::Full) => RequestedAccess::Full,
        None => return Err(std::io::Error::other("account grant plan is incomplete").into()),
    };
    let profile = negotiate_profile(&ceylith, &client_runtime, &config, requested_access).await?;
    eprintln!("Lirvena accepted Ceylith Profile");

    let mut watch_runtime = if requested_access == RequestedAccess::Full {
        let watch_connection = connect(&config, &client_runtime).await?;
        ensure_matching_admission(ceylith.admission(), watch_connection.client().admission())?;
        Some(spawn_installation_watch(watch_connection, 8)?)
    } else {
        None
    };
    let account_runtime = spawn_account(
        AccountRuntimeConfig::new(config.state_directory.clone(), account_local_id, 64)?,
        now_ms()?,
    )
    .await?;
    let account = account_runtime.handle();
    account
        .transition(AccountTransition {
            next: AccountPhase::Starting,
            protective_reason: None,
            occurred_at_ms: now_ms()?,
        })
        .await?;
    let _delivered = event_publisher.publish(AccountEvent::Lifecycle {
        local_id: account_local_id,
        phase: AccountPhase::Starting,
        protective_reason: None,
        occurred_at_ms: now_ms()?,
    });

    let mut login = Box::pin(flow::run(
        &config,
        &ceylith,
        &profile,
        &account,
        &event_publisher,
    ));
    let (result, protective_reason) = if let Some(watch_runtime) = watch_runtime.as_mut() {
        loop {
            tokio::select! {
                result = &mut login => break (result, Some(ProtectiveReason::WorkerFailure)),
                watched = watch_runtime.watch().next() => {
                    match watched {
                        Some(Ok(event)) => match classify(&event, account_local_id, &grant_plan) {
                            ContinuityAction::Continue => {
                                enqueue_watch_notification(notification_handle.as_ref(), &event, account_local_id);
                                eprintln!("Lirvena received Ceylith {:?} continuity event", event.kind());
                            }
                            ContinuityAction::Protect(reason) => {
                                enqueue_watch_notification(notification_handle.as_ref(), &event, account_local_id);
                                break (Err(std::io::Error::other("Ceylith required protective offline").into()), Some(reason));
                            }
                        },
                        Some(Err(error)) => break (Err(error.into()), Some(ProtectiveReason::CeylithContinuity)),
                        None => break (Err(std::io::Error::other("Ceylith Watch ended").into()), Some(ProtectiveReason::CeylithContinuity)),
                    }
                }
            }
        }
    } else {
        (login.as_mut().await, Some(ProtectiveReason::WorkerFailure))
    };
    drop(login);

    let terminal = match &result {
        Ok(()) => AccountTransition {
            next: AccountPhase::Stopped,
            protective_reason: None,
            occurred_at_ms: now_ms()?,
        },
        Err(_error) => AccountTransition {
            next: AccountPhase::ProtectiveOffline,
            protective_reason,
            occurred_at_ms: now_ms()?,
        },
    };
    let mut cleanup_error: Option<Box<dyn std::error::Error>> = None;
    if let Err(error) = account.transition(terminal).await {
        cleanup_error = Some(error.into());
    } else {
        let _delivered = event_publisher.publish(AccountEvent::Lifecycle {
            local_id: account_local_id,
            phase: terminal.next,
            protective_reason: terminal.protective_reason,
            occurred_at_ms: terminal.occurred_at_ms,
        });
    }
    if result.is_err()
        && let Some(reason) = protective_reason
        && let Ok(event) =
            notification::protective_offline_event(now_ms()?, account_local_id, reason)
    {
        enqueue_notification(notification_handle.as_ref(), event);
    }
    if let Some(watch_runtime) = watch_runtime
        && let Err(error) = watch_runtime.shutdown().await
    {
        cleanup_error.get_or_insert_with(|| error.into());
    }
    if let Err(error) = ceylith_runtime.shutdown().await {
        cleanup_error.get_or_insert_with(|| error.into());
    }
    if let Err(error) = account_runtime.shutdown().await {
        cleanup_error.get_or_insert_with(|| error.into());
    }
    if let Some(center) = notification_center
        && let Err(error) = center.shutdown().await
    {
        cleanup_error.get_or_insert_with(|| error.into());
    }
    match result {
        Err(error) => Err(error),
        Ok(()) => match cleanup_error {
            Some(error) => Err(error),
            None => Ok(()),
        },
    }
}

fn enqueue_watch_notification(
    handle: Option<&NotificationHandle>,
    event: &ceylith_protocol::WatchEvent,
    account: AccountLocalId,
) {
    if let Ok(notification) = notification::watch_event(event, account) {
        enqueue_notification(handle, notification);
    }
}

fn enqueue_notification(
    handle: Option<&NotificationHandle>,
    event: notify_runtime::NotificationEvent,
) {
    let Some(handle) = handle else {
        return;
    };
    let enqueued_at_ms = now_ms().unwrap_or_else(|_| event.occurred_at_ms());
    if handle.try_enqueue(event, enqueued_at_ms).is_err() {
        eprintln!("WARNING: Lirvena could not persist an operational notification");
    }
}

const fn grant_availability(admission: &SessionAdmission) -> GrantAvailability {
    match admission.grant_class() {
        GrantClass::Public => GrantAvailability::PublicOnly,
        GrantClass::Full if admission.max_full_accounts() == 0 => GrantAvailability::UnboundedFull,
        GrantClass::Community | GrantClass::Full => GrantAvailability::BoundedFull {
            max_accounts: admission.max_full_accounts(),
        },
    }
}
use account_api::{AccountEvent, AccountEventHub};
