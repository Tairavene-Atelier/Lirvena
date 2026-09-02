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
        events, actions, dispatcher, event_bus, reporters, receiver,
    ));
    Ok(Some(OneBotRuntime { shutdown, tasks }))
}

async fn coordinate(
    mut events: AccountEventSubscription,
    actions: BTreeMap<AccountLocalId, AccountActionHandle>,
    dispatcher: Arc<OneBotDispatcher>,
    event_bus: Arc<OneBotEventBus>,
    reporters: Vec<HttpEventReporter>,
    mut shutdown: watch::Receiver<bool>,
) -> Result<(), io::Error> {
    let mut identities = BTreeMap::<AccountLocalId, u64>::new();
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
                if let Ok(Some(mut projected)) = project_account_event(&event, IdFormat::String) {
                    attach_lifecycle_identity(&event, &identities, &mut projected);
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
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() {
                    return Ok(());
                }
            }
        }
    }
}

fn attach_lifecycle_identity(
    event: &AccountEvent,
    identities: &BTreeMap<AccountLocalId, u64>,
    projected: &mut Value,
) {
    let AccountEvent::Lifecycle { local_id, .. } = event else {
        return;
    };
    let Some(self_id) = identities.get(local_id) else {
        return;
    };
    if let Some(object) = projected.as_object_mut() {
        object.insert("self_id".to_owned(), Value::String(self_id.to_string()));
    }
}

async fn wait_for_shutdown(receiver: &mut watch::Receiver<bool>) {
    while !*receiver.borrow() {
        if receiver.changed().await.is_err() {
            return;
        }
    }
}
