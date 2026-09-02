use std::collections::BTreeMap;
use std::io;

use account_api::{AccountEvent, AccountEventHub, AccountEventPublisher};
use account_runtime::{
    AccountGrantRequest, AccountLocalId, AccountPhase, AccountRuntimeConfig, AccountSupervisor,
    AccountTransition, AssignedRealm, GrantAvailability, GrantPlan, ProtectiveReason,
    plan_account_grants,
};
use ceylith_client::{
    InstallationClient, InstallationWatchRuntime, RequestedAccess, spawn_installation_watch,
};
use ceylith_protocol::{GrantClass, SessionAdmission, WatchEvent};
use notify_runtime::NotificationHandle;
use qq_profile::LinuxNtProfile;
use tokio::sync::watch;
use tokio::task::JoinSet;

use super::ceylith::{connect, ensure_matching_admission, negotiate_profile, runtime};
use super::continuity::{ContinuityAction, classify};
use super::flow;
use crate::config::{AccountConfig, ProcessConfig};
use crate::notification;
use crate::support::now_ms;

const ACCOUNT_QUEUE_CAPACITY: usize = 64;
const EVENT_QUEUE_CAPACITY: usize = 1_024;
const WATCH_QUEUE_CAPACITY: usize = 8;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StopDirective {
    Running,
    Graceful,
    Protective(ProtectiveReason),
}

struct AccountCompletion {
    local_id: AccountLocalId,
    result: Result<(), io::Error>,
}

pub(super) async fn run(config: ProcessConfig) -> Result<(), Box<dyn std::error::Error>> {
    let account_events = AccountEventHub::new(EVENT_QUEUE_CAPACITY)?;
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

    let grant_plan = plan(&config, ceylith.admission())?;
    report_fallbacks(&grant_plan);
    let profiles = Profiles::negotiate(&config, &ceylith, &client_runtime, &grant_plan).await?;
    let mut watch_runtime = start_watch(&config, &client_runtime, &ceylith, &grant_plan).await?;
    let mut supervisor = AccountSupervisor::new();
    let mut jobs = JoinSet::new();
    let mut stop_channels = BTreeMap::new();

    for account_config in &config.accounts {
        let local_id = AccountLocalId::from_bytes(account_config.account_slot_id);
        let handle = supervisor
            .spawn(
                AccountRuntimeConfig::new(
                    config.state_directory.clone(),
                    local_id,
                    ACCOUNT_QUEUE_CAPACITY,
                )?,
                now_ms()?,
            )
            .await?;
        transition_and_publish(&handle, &event_publisher, AccountPhase::Starting, None).await?;
        let realm = grant_plan
            .assigned_realm(local_id)
            .ok_or_else(|| io::Error::other("account grant plan is incomplete"))?;
        let profile = profiles.for_realm(realm)?.clone();
        let (stop_sender, stop_receiver) = watch::channel(StopDirective::Running);
        stop_channels.insert(local_id, stop_sender);
        jobs.spawn(run_account(
            account_config.clone(),
            ceylith.clone(),
            profile,
            handle,
            event_publisher.clone(),
            notification_handle.clone(),
            stop_receiver,
        ));
    }

    let mut first_error = supervise(
        &mut jobs,
        &mut watch_runtime,
        &grant_plan,
        &stop_channels,
        notification_handle.as_ref(),
    )
    .await;
    if let Some(watch) = watch_runtime
        && let Err(error) = watch.shutdown().await
    {
        first_error.get_or_insert_with(|| io::Error::other(error.to_string()));
    }
    if let Err(error) = ceylith_runtime.shutdown().await {
        first_error.get_or_insert_with(|| io::Error::other(error.to_string()));
    }
    if let Err(error) = supervisor.shutdown_all().await {
        first_error.get_or_insert_with(|| io::Error::other(error.to_string()));
    }
    if let Some(center) = notification_center
        && let Err(error) = center.shutdown().await
    {
        first_error.get_or_insert_with(|| io::Error::other(error.to_string()));
    }
    first_error.map_or(Ok(()), |error| Err(error.into()))
}

async fn run_account(
    config: AccountConfig,
    ceylith: InstallationClient,
    profile: LinuxNtProfile,
    account: account_runtime::AccountHandle,
    events: AccountEventPublisher,
    notifications: Option<NotificationHandle>,
    mut stop: watch::Receiver<StopDirective>,
) -> AccountCompletion {
    let local_id = account.local_id();
    let outcome = tokio::select! {
        result = flow::run(&config, &ceylith, &profile, &account, &events) => {
            result.map_err(|error| io::Error::other(error.to_string()))
        }
        directive = wait_for_stop(&mut stop) => match directive {
            StopDirective::Graceful => Ok(()),
            StopDirective::Protective(reason) => {
                Err(io::Error::other(format!("account entered protective offline: {reason:?}")))
            }
            StopDirective::Running => Err(io::Error::other("account stop channel ended")),
        }
    };
    let directive = *stop.borrow();
    let (phase, reason) = match (&outcome, directive) {
        (Ok(()), _) | (_, StopDirective::Graceful) => (AccountPhase::Stopped, None),
        (_, StopDirective::Protective(reason)) => (AccountPhase::ProtectiveOffline, Some(reason)),
        (Err(_), StopDirective::Running) => (
            AccountPhase::ProtectiveOffline,
            Some(ProtectiveReason::WorkerFailure),
        ),
    };
    let transition = transition_and_publish(&account, &events, phase, reason).await;
    if let Some(reason) = reason
        && let Ok(event) =
            notification::protective_offline_event(now_ms().unwrap_or_default(), local_id, reason)
    {
        enqueue_notification(notifications.as_ref(), event);
    }
    let result = match (outcome, transition) {
        (Err(error), _) => Err(error),
        (Ok(()), Err(error)) => Err(io::Error::other(error.to_string())),
        (Ok(()), Ok(())) => Ok(()),
    };
    AccountCompletion { local_id, result }
}

async fn supervise(
    jobs: &mut JoinSet<AccountCompletion>,
    watch_runtime: &mut Option<InstallationWatchRuntime>,
    plan: &GrantPlan,
    stop_channels: &BTreeMap<AccountLocalId, watch::Sender<StopDirective>>,
    notifications: Option<&NotificationHandle>,
) -> Option<io::Error> {
    let mut first_error = None;
    while !jobs.is_empty() {
        tokio::select! {
            joined = jobs.join_next() => {
                match joined {
                    Some(Ok(completion)) => if let Err(error) = completion.result {
                        eprintln!("Lirvena account {:?} stopped: {error}", completion.local_id);
                        first_error.get_or_insert(error);
                    },
                    Some(Err(error)) => {
                        first_error.get_or_insert_with(|| io::Error::other(error.to_string()));
                    }
                    None => break,
                }
            }
            watched = next_watch(watch_runtime), if watch_runtime.is_some() => {
                match watched {
                    Some(Ok(event)) => handle_watch(&event, plan, stop_channels, notifications),
                    Some(Err(error)) => {
                        protect_full(plan, stop_channels, ProtectiveReason::CeylithContinuity);
                        first_error.get_or_insert_with(|| io::Error::other(error.to_string()));
                        *watch_runtime = None;
                    }
                    None => {
                        protect_full(plan, stop_channels, ProtectiveReason::CeylithContinuity);
                        first_error.get_or_insert_with(|| io::Error::other("Ceylith Watch ended"));
                        *watch_runtime = None;
                    }
                }
            }
            signal = tokio::signal::ctrl_c() => {
                if let Err(error) = signal {
                    first_error.get_or_insert(error);
                }
                for sender in stop_channels.values() {
                    let _changed = sender.send(StopDirective::Graceful);
                }
            }
        }
    }
    first_error
}

async fn next_watch(
    runtime: &mut Option<InstallationWatchRuntime>,
) -> Option<Result<WatchEvent, ceylith_client::ClientError>> {
    match runtime {
        Some(value) => value.watch().next().await,
        None => std::future::pending().await,
    }
}

async fn wait_for_stop(receiver: &mut watch::Receiver<StopDirective>) -> StopDirective {
    loop {
        if *receiver.borrow() != StopDirective::Running {
            return *receiver.borrow();
        }
        if receiver.changed().await.is_err() {
            return StopDirective::Running;
        }
    }
}

fn handle_watch(
    event: &WatchEvent,
    plan: &GrantPlan,
    stop_channels: &BTreeMap<AccountLocalId, watch::Sender<StopDirective>>,
    notifications: Option<&NotificationHandle>,
) {
    for (local_id, sender) in stop_channels {
        if plan.assigned_realm(*local_id) != Some(AssignedRealm::Full) {
            continue;
        }
        match classify(event, *local_id, plan) {
            ContinuityAction::Continue => {
                enqueue_watch_notification(notifications, event, *local_id);
            }
            ContinuityAction::Protect(reason) => {
                enqueue_watch_notification(notifications, event, *local_id);
                let _changed = sender.send(StopDirective::Protective(reason));
            }
        }
    }
}

fn protect_full(
    plan: &GrantPlan,
    stop_channels: &BTreeMap<AccountLocalId, watch::Sender<StopDirective>>,
    reason: ProtectiveReason,
) {
    for local_id in plan.protective_offline_on_revocation() {
        if let Some(sender) = stop_channels.get(&local_id) {
            let _changed = sender.send(StopDirective::Protective(reason));
        }
    }
}

async fn transition_and_publish(
    account: &account_runtime::AccountHandle,
    events: &AccountEventPublisher,
    phase: AccountPhase,
    protective_reason: Option<ProtectiveReason>,
) -> Result<(), account_runtime::AccountRuntimeError> {
    let occurred_at_ms = now_ms().unwrap_or_default();
    account
        .transition(AccountTransition {
            next: phase,
            protective_reason,
            occurred_at_ms,
        })
        .await?;
    let _delivered = events.publish(AccountEvent::Lifecycle {
        local_id: account.local_id(),
        phase,
        protective_reason,
        occurred_at_ms,
    });
    Ok(())
}

fn plan(config: &ProcessConfig, admission: &SessionAdmission) -> Result<GrantPlan, io::Error> {
    plan_account_grants(
        config.accounts.iter().map(|account| {
            AccountGrantRequest::new(
                AccountLocalId::from_bytes(account.account_slot_id),
                account.account_mode,
            )
        }),
        grant_availability(admission),
    )
    .map_err(|error| io::Error::other(format_grant_error(&error)))
}

fn format_grant_error(error: &account_runtime::GrantPlanError) -> String {
    match error {
        account_runtime::GrantPlanError::DuplicateAccount { account } => {
            format!("duplicate account configuration: {account:?}")
        }
        account_runtime::GrantPlanError::GrantRequired { accounts } => {
            format!("Full grant required by accounts: {accounts:?}")
        }
        account_runtime::GrantPlanError::FullQuotaExceeded { limit, accounts } => format!(
            "Full account quota {limit} is smaller than configured accounts {accounts:?}; explicitly change selected accounts to public"
        ),
    }
}

fn report_fallbacks(plan: &GrantPlan) {
    for account in plan.public_fallbacks() {
        eprintln!(
            "WARNING: Lirvena account {account:?} is starting in Public mode because Full is unavailable"
        );
    }
}

async fn start_watch(
    config: &ProcessConfig,
    runtime: &ceylith_client::RuntimeDescriptor,
    operations: &InstallationClient,
    plan: &GrantPlan,
) -> Result<Option<InstallationWatchRuntime>, Box<dyn std::error::Error>> {
    if plan.protective_offline_on_revocation().is_empty() {
        return Ok(None);
    }
    let connection = connect(config, runtime).await?;
    ensure_matching_admission(operations.admission(), connection.client().admission())?;
    Ok(Some(spawn_installation_watch(
        connection,
        WATCH_QUEUE_CAPACITY,
    )?))
}

struct Profiles {
    public: Option<LinuxNtProfile>,
    full: Option<LinuxNtProfile>,
}

impl Profiles {
    async fn negotiate(
        config: &ProcessConfig,
        ceylith: &InstallationClient,
        runtime: &ceylith_client::RuntimeDescriptor,
        plan: &GrantPlan,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let needs_public = config.accounts.iter().any(|account| {
            plan.assigned_realm(AccountLocalId::from_bytes(account.account_slot_id))
                == Some(AssignedRealm::Public)
        });
        let needs_full = !plan.protective_offline_on_revocation().is_empty();
        let public = if needs_public {
            Some(negotiate_profile(ceylith, runtime, config, RequestedAccess::Public).await?)
        } else {
            None
        };
        let full = if needs_full {
            Some(negotiate_profile(ceylith, runtime, config, RequestedAccess::Full).await?)
        } else {
            None
        };
        eprintln!("Lirvena accepted every required Ceylith Profile");
        Ok(Self { public, full })
    }

    fn for_realm(&self, realm: AssignedRealm) -> Result<&LinuxNtProfile, io::Error> {
        match realm {
            AssignedRealm::Public => self.public.as_ref(),
            AssignedRealm::Full => self.full.as_ref(),
        }
        .ok_or_else(|| io::Error::other("required Ceylith Profile is missing"))
    }
}

fn enqueue_watch_notification(
    handle: Option<&NotificationHandle>,
    event: &WatchEvent,
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
