use std::collections::{BTreeMap, BTreeSet};

use account_api::{AccountEvent, AccountEventSubscription, EventHubError};
use account_runtime::{AccountLocalId, AccountPhase};
use ceylith_client::{
    CommunityTelemetrySigner, CommunityTelemetrySpec, InstallationClient, RuntimeDescriptor,
};
use ceylith_protocol::{Digest32, ProfileId, TelemetryReportId};
use community_telemetry::{CommunityTelemetryStore, TelemetryStoreError};
use sha2::{Digest, Sha256};
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use tokio::time::{Duration, MissedTickBehavior};

use crate::support::now_ms;

const FLUSH_INTERVAL: Duration = Duration::from_mins(1);
const MAX_REPORTS_PER_FLUSH: usize = 8;

pub(crate) struct CommunityTelemetryRuntime {
    shutdown: Option<oneshot::Sender<()>>,
    task: Option<JoinHandle<()>>,
}

impl CommunityTelemetryRuntime {
    pub(crate) fn start(
        setup: CommunityTelemetrySetup,
        events: AccountEventSubscription,
    ) -> Result<Self, TelemetryStoreError> {
        let opened_at_ms = now_ms().map_err(|_error| TelemetryStoreError::InvalidInput)?;
        let mut store = CommunityTelemetryStore::open(&setup.state_directory, opened_at_ms)?;
        store.observe_account_set(opened_at_ms, setup.account_set_digest)?;
        let (shutdown, receiver) = oneshot::channel();
        let task = tokio::spawn(run(setup, store, events, receiver));
        Ok(Self {
            shutdown: Some(shutdown),
            task: Some(task),
        })
    }

    pub(crate) async fn shutdown(mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ignored = shutdown.send(());
        }
        if let Some(task) = self.task.take()
            && task.await.is_err()
        {
            eprintln!("WARNING: Lirvena Community telemetry worker failed");
        }
    }
}

impl Drop for CommunityTelemetryRuntime {
    fn drop(&mut self) {
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

pub(crate) struct CommunityTelemetrySetup {
    state_directory: std::path::PathBuf,
    ceylith: InstallationClient,
    signer: CommunityTelemetrySigner,
    installation_id: [u8; 16],
    account_set_digest: [u8; 32],
    profile_id: ProfileId,
    profile_manifest_digest: Digest32,
    build_digest: Digest32,
    platform: u32,
    architecture: u32,
}

impl CommunityTelemetrySetup {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        state_directory: std::path::PathBuf,
        ceylith: InstallationClient,
        signing_seed: [u8; 32],
        installation_id: [u8; 16],
        account_slots: impl IntoIterator<Item = [u8; 16]>,
        profile_id: ProfileId,
        profile_manifest_digest: Digest32,
        runtime: &RuntimeDescriptor,
    ) -> Result<Self, ceylith_client::ClientError> {
        Ok(Self {
            state_directory,
            ceylith,
            signer: CommunityTelemetrySigner::from_seed(signing_seed),
            installation_id,
            account_set_digest: account_set_digest(account_slots),
            profile_id,
            profile_manifest_digest,
            build_digest: runtime.build_digest()?,
            platform: runtime.platform(),
            architecture: runtime.architecture(),
        })
    }

    fn spec(
        &self,
        day: community_telemetry::CompletedDay,
        generated_at_ms: u64,
    ) -> CommunityTelemetrySpec {
        CommunityTelemetrySpec {
            report_id: report_id(self.installation_id, day.utc_day),
            utc_day: day.utc_day,
            group_count: day.group_count,
            messages_received: day.messages_received,
            messages_sent: day.messages_sent,
            active_duration: day.active_duration,
            profile_id: self.profile_id,
            profile_manifest_digest: self.profile_manifest_digest,
            build_digest: self.build_digest,
            platform: self.platform,
            architecture: self.architecture,
            account_churn: day.account_churn,
            generated_at_ms,
        }
    }
}

async fn run(
    setup: CommunityTelemetrySetup,
    mut store: CommunityTelemetryStore,
    mut events: AccountEventSubscription,
    mut shutdown: oneshot::Receiver<()>,
) {
    let mut active_accounts = BTreeSet::new();
    let mut group_counts = BTreeMap::new();
    let mut interval = tokio::time::interval(FLUSH_INTERVAL);
    interval.set_missed_tick_behavior(MissedTickBehavior::Delay);
    loop {
        tokio::select! {
            biased;
            _result = &mut shutdown => {
                close_active_accounts(&mut store, &mut active_accounts);
                break;
            }
            _instant = interval.tick() => flush(&setup, &mut store).await,
            event = events.receive() => match event {
                Ok(event) => record_event(
                    &mut store,
                    &mut active_accounts,
                    &mut group_counts,
                    &event,
                ),
                Err(EventHubError::Lagged) => {
                    eprintln!("WARNING: Lirvena Community telemetry skipped lagged soft signals");
                }
                Err(EventHubError::Closed) => break,
                Err(EventHubError::InvalidCapacity | EventHubError::InvalidEvent) => {
                    eprintln!("WARNING: Lirvena Community telemetry rejected an internal event");
                }
            }
        }
    }
}

fn record_event(
    store: &mut CommunityTelemetryStore,
    active_accounts: &mut BTreeSet<AccountLocalId>,
    group_counts: &mut BTreeMap<AccountLocalId, u64>,
    event: &AccountEvent,
) {
    let result = match event {
        AccountEvent::IdentityReady(_) | AccountEvent::GroupNotice(_) => Ok(()),
        AccountEvent::Message(_) => now_ms()
            .map_err(|_error| TelemetryStoreError::InvalidInput)
            .and_then(|time| store.record_received(time)),
        AccountEvent::OutboundMessageAccepted { occurred_at_ms, .. } => {
            store.record_sent(*occurred_at_ms)
        }
        AccountEvent::GroupCountObserved {
            local_id,
            count,
            occurred_at_ms,
        } => {
            group_counts.insert(*local_id, *count);
            group_counts
                .values()
                .try_fold(0_u64, |total, value| total.checked_add(*value))
                .ok_or(TelemetryStoreError::InvalidInput)
                .and_then(|total| store.observe_group_count(*occurred_at_ms, total))
        }
        AccountEvent::Lifecycle {
            local_id,
            phase,
            occurred_at_ms,
            ..
        } => record_lifecycle(store, active_accounts, *local_id, *phase, *occurred_at_ms),
    };
    if result.is_err() {
        eprintln!("WARNING: Lirvena could not persist one Community telemetry soft signal");
    }
}

fn record_lifecycle(
    store: &mut CommunityTelemetryStore,
    active_accounts: &mut BTreeSet<AccountLocalId>,
    local_id: AccountLocalId,
    phase: AccountPhase,
    occurred_at_ms: u64,
) -> Result<(), TelemetryStoreError> {
    if phase == AccountPhase::Active {
        if active_accounts.insert(local_id) {
            store.set_account_active(occurred_at_ms, true)?;
        }
    } else if active_accounts.remove(&local_id) {
        store.set_account_active(occurred_at_ms, false)?;
    }
    Ok(())
}

async fn flush(setup: &CommunityTelemetrySetup, store: &mut CommunityTelemetryStore) {
    let Ok(generated_at_ms) = now_ms() else {
        eprintln!("WARNING: Lirvena could not read time for Community telemetry");
        return;
    };
    if store.checkpoint_activity(generated_at_ms).is_err() {
        eprintln!("WARNING: Lirvena could not checkpoint Community activity");
        return;
    }
    for _attempt in 0..MAX_REPORTS_PER_FLUSH {
        let day = match store.oldest_pending(generated_at_ms) {
            Ok(Some(day)) => day,
            Ok(None) => return,
            Err(_error) => {
                eprintln!("WARNING: Lirvena could not read pending Community telemetry");
                return;
            }
        };
        let spec = setup.spec(day, generated_at_ms);
        if setup
            .ceylith
            .submit_community_telemetry(&setup.signer, spec)
            .await
            .is_err()
        {
            eprintln!("WARNING: Ceylith did not accept pending Community telemetry");
            return;
        }
        if store.mark_sent(day.utc_day, generated_at_ms).is_err() {
            eprintln!("WARNING: Lirvena could not mark Community telemetry delivered");
            return;
        }
    }
}

fn close_active_accounts(
    store: &mut CommunityTelemetryStore,
    active_accounts: &mut BTreeSet<AccountLocalId>,
) {
    let time = now_ms().unwrap_or_default();
    while active_accounts.pop_first().is_some() {
        if store.set_account_active(time, false).is_err() {
            eprintln!("WARNING: Lirvena could not close Community activity state");
            break;
        }
    }
}

fn account_set_digest(slots: impl IntoIterator<Item = [u8; 16]>) -> [u8; 32] {
    let mut slots: Vec<_> = slots.into_iter().collect();
    slots.sort_unstable();
    let mut hasher = Sha256::new();
    hasher.update(b"lirvena-community-account-set-v1");
    for slot in slots {
        hasher.update(slot);
    }
    hasher.finalize().into()
}

fn report_id(installation_id: [u8; 16], utc_day: u32) -> TelemetryReportId {
    let mut hasher = Sha256::new();
    hasher.update(b"lirvena-community-report-v1");
    hasher.update(installation_id);
    hasher.update(utc_day.to_be_bytes());
    let digest: [u8; 32] = hasher.finalize().into();
    let mut id = [0_u8; 16];
    id.copy_from_slice(&digest[..16]);
    TelemetryReportId::from_bytes(id)
}
