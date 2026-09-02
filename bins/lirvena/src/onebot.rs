use std::collections::BTreeMap;
use std::io;
use std::sync::Arc;

use account_api::{AccountActionHandle, AccountEvent, AccountEventSubscription};
use account_runtime::{AccountLocalId, AccountPhase};
use adapter_onebot::{
    AccountChannelBackend, DispatcherConfig, ForwardServerConfig, HttpEventReporter, IdFormat,
    OneBotDispatcher, OneBotEventBus, OneBotForwardServer, ReverseWebSocket, project_account_event,
};
use serde_json::Value;
use tokio::sync::watch;
use tokio::task::JoinSet;
use tokio::time::MissedTickBehavior;

use crate::config::OneBotConfig;

const ACTION_QUEUE_CAPACITY: usize = 1_024;

pub(super) struct OneBotRuntime {
    shutdown: watch::Sender<bool>,
    tasks: JoinSet<Result<(), io::Error>>,
}

impl OneBotRuntime {
    pub(super) async fn shutdown(mut self) -> Result<(), io::Error> {
        let _changed = self.shutdown.send(true);
        let mut first_error = None;
        while let Some(joined) = self.tasks.join_next().await {
            match joined {
                Ok(Ok(())) => {}
                Ok(Err(error)) => {
                    first_error.get_or_insert(error);
                }
                Err(error) => {
                    first_error.get_or_insert_with(|| io::Error::other(error.to_string()));
                }
            }
        }
        first_error.map_or(Ok(()), Err)
    }
}

pub(super) async fn start(
    config: Option<&OneBotConfig>,
    events: AccountEventSubscription,
    actions: BTreeMap<AccountLocalId, AccountActionHandle>,
) -> Result<Option<OneBotRuntime>, io::Error> {
    let Some(config) = config else {
        return Ok(None);
    };
    let dispatcher = Arc::new(
        OneBotDispatcher::empty(DispatcherConfig {
            bound_self_id: None,
            queue_capacity: ACTION_QUEUE_CAPACITY,
            id_format: config.id_format,
        })
        .map_err(|error| io::Error::other(error.to_string()))?,
    );
    let event_bus = Arc::new(
        OneBotEventBus::new(config.event_queue_capacity)
            .map_err(|error| io::Error::other(error.to_string()))?,
    );
    let reporters = config
        .http_post
        .iter()
        .cloned()
        .map(HttpEventReporter::new)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| io::Error::other(error.to_string()))?;
    let reverse_websockets = config
        .reverse_websocket
        .iter()
        .cloned()
        .map(ReverseWebSocket::new)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| io::Error::other(error.to_string()))?;
    let server = OneBotForwardServer::bind(
        ForwardServerConfig {
            listen: config.forward_listen,
            access_token: config.access_token.as_ref().map(|token| token.to_vec()),
            max_body_bytes: config.max_body_bytes,
        },
        dispatcher.clone(),
        event_bus.clone(),
    )
    .await
    .map_err(|error| io::Error::other(error.to_string()))?;
    eprintln!("Lirvena OneBot is listening on {}", server.local_addr()?);
    let (shutdown, receiver) = watch::channel(false);
    let mut tasks = JoinSet::new();
    let mut server_shutdown = receiver.clone();
    tasks.spawn(async move {
        server
            .serve(async move {
                wait_for_shutdown(&mut server_shutdown).await;
            })
            .await
            .map_err(|error| io::Error::other(error.to_string()))
    });
    for reverse in reverse_websockets {
        let reverse_dispatcher = dispatcher.clone();
        let reverse_events = event_bus.clone();
        let reverse_shutdown = receiver.clone();
        tasks.spawn(async move {
            reverse
                .run(reverse_dispatcher, reverse_events, reverse_shutdown)
                .await;
            Ok(())
        });
    }
    tasks.spawn(coordinate(
        EventCoordinator {
            events,
            actions,
            dispatcher,
            event_bus,
            reporters,
            id_format: config.id_format,
            heartbeat_interval: config.heartbeat_interval,
        },
        receiver,
    ));
    Ok(Some(OneBotRuntime { shutdown, tasks }))
}

struct EventCoordinator {
    events: AccountEventSubscription,
    actions: BTreeMap<AccountLocalId, AccountActionHandle>,
    dispatcher: Arc<OneBotDispatcher>,
    event_bus: Arc<OneBotEventBus>,
    reporters: Vec<HttpEventReporter>,
    id_format: IdFormat,
    heartbeat_interval: std::time::Duration,
}

async fn coordinate(
    coordinator: EventCoordinator,
    mut shutdown: watch::Receiver<bool>,
) -> Result<(), io::Error> {
    let EventCoordinator {
        mut events,
        actions,
        dispatcher,
        event_bus,
        reporters,
        id_format,
        heartbeat_interval,
    } = coordinator;
    let mut identities = BTreeMap::<AccountLocalId, u64>::new();
    let mut heartbeat = tokio::time::interval(heartbeat_interval);
    heartbeat.set_missed_tick_behavior(MissedTickBehavior::Skip);
    heartbeat.tick().await;
    loop {
        tokio::select! {
            event = events.receive() => {
                let event = event.map_err(|error| io::Error::other(error.to_string()))?;
                if let AccountEvent::IdentityReady(identity) = &event {
                    let handle = actions.get(&identity.local_id())
                        .ok_or_else(|| io::Error::other("OneBot account action handle is missing"))?;
                    dispatcher.register(
                        identity.qq_id(),
                        Arc::new(AccountChannelBackend::new(handle.clone())),
                    ).map_err(|error| io::Error::other(error.to_string()))?;
                    identities.insert(identity.local_id(), identity.qq_id());
                }
                if let Ok(Some(mut projected)) = project_account_event(&event, id_format) {
                    attach_lifecycle_identity(&event, &identities, id_format, &mut projected);
                    for reporter in &reporters {
                        let _result = reporter.report_and_handle(&projected, &dispatcher).await;
                    }
                    let _delivered = event_bus.publish(projected);
                }
                if let AccountEvent::Lifecycle { local_id, phase, .. } = event
                    && matches!(phase, AccountPhase::Stopped | AccountPhase::ProtectiveOffline)
                    && let Some(self_id) = identities.remove(&local_id)
                {
                    let _removed = dispatcher.unregister(self_id);
                }
            }
            _ = heartbeat.tick() => {
                for self_id in identities.values() {
                    let event = heartbeat_event(*self_id, id_format, heartbeat_interval)?;
                    for reporter in &reporters {
                        let _result = reporter.report_and_handle(&event, &dispatcher).await;
                    }
                    let _delivered = event_bus.publish(event);
                }
            }
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() {
                    return Ok(());
                }
            }
        }
    }
}

fn heartbeat_event(
    self_id: u64,
    id_format: IdFormat,
    interval: std::time::Duration,
) -> Result<Value, io::Error> {
    let interval = u64::try_from(interval.as_millis())
        .map_err(|_error| io::Error::other("OneBot heartbeat interval overflow"))?;
    let self_id = match id_format {
        IdFormat::String => Value::String(self_id.to_string()),
        IdFormat::Number => serde_json::json!(self_id),
    };
    Ok(serde_json::json!({
        "time": crate::support::now_seconds()?,
        "self_id": self_id,
        "post_type": "meta_event",
        "meta_event_type": "heartbeat",
        "status": {"online": true, "good": true},
        "interval": interval,
    }))
}

fn attach_lifecycle_identity(
    event: &AccountEvent,
    identities: &BTreeMap<AccountLocalId, u64>,
    id_format: IdFormat,
    projected: &mut Value,
) {
    let AccountEvent::Lifecycle { local_id, .. } = event else {
        return;
    };
    let Some(self_id) = identities.get(local_id) else {
        return;
    };
    if let Some(object) = projected.as_object_mut() {
        let value = match id_format {
            IdFormat::String => Value::String(self_id.to_string()),
            IdFormat::Number => serde_json::json!(self_id),
        };
        object.insert("self_id".to_owned(), value);
    }
}

async fn wait_for_shutdown(receiver: &mut watch::Receiver<bool>) {
    while !*receiver.borrow() {
        if receiver.changed().await.is_err() {
            return;
        }
    }
}
