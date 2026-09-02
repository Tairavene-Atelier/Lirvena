use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};

use tokio::sync::{Mutex, mpsc};

use crate::quick::expand_quick_operation;
use crate::{ActionMode, ActionRequest, ActionResponse, BackendError, IdFormat, OneBotBackend};

const MAX_QUEUE_CAPACITY: usize = 4_096;

/// `OneBot` account routing and asynchronous queue configuration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DispatcherConfig {
    /// Optional account fixed to this endpoint.
    pub bound_self_id: Option<u64>,
    /// Bounded asynchronous action capacity.
    pub queue_capacity: usize,
    /// Identifier representation applied to action response data.
    pub id_format: IdFormat,
}

/// Shared `OneBot` action router for every transport.
pub struct OneBotDispatcher {
    accounts: RwLock<BTreeMap<u64, Arc<dyn OneBotBackend>>>,
    bound_self_id: Option<u64>,
    asynchronous: mpsc::Sender<QueuedAction>,
    rate_limited: Arc<Mutex<()>>,
    id_format: IdFormat,
}

struct QueuedAction {
    backend: Arc<dyn OneBotBackend>,
    request: ActionRequest,
}

impl OneBotDispatcher {
    /// Creates an account router and its single bounded asynchronous lane.
    ///
    /// # Errors
    ///
    /// Returns an error for empty accounts, zero IDs, duplicate IDs, an unknown endpoint binding,
    /// or an invalid queue capacity.
    pub fn new(
        accounts: impl IntoIterator<Item = (u64, Arc<dyn OneBotBackend>)>,
        config: DispatcherConfig,
    ) -> Result<Self, BackendError> {
        if config.queue_capacity == 0 || config.queue_capacity > MAX_QUEUE_CAPACITY {
            return Err(BackendError::BadParameters(
                "OneBot queue capacity is invalid".to_owned(),
            ));
        }
        let mut indexed = BTreeMap::new();
        for (self_id, backend) in accounts {
            if self_id == 0 || indexed.insert(self_id, backend).is_some() {
                return Err(BackendError::BadParameters(
                    "OneBot account routing is invalid".to_owned(),
                ));
            }
        }
        if indexed.is_empty()
            || config
                .bound_self_id
                .is_some_and(|self_id| !indexed.contains_key(&self_id))
        {
            return Err(BackendError::BadParameters(
                "OneBot endpoint account binding is invalid".to_owned(),
            ));
        }
        Ok(Self::build(indexed, config))
    }

    /// Creates an initially empty dynamic account router.
    ///
    /// Accounts may register after QQ publishes its authenticated identity.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid queue capacity.
    pub fn empty(config: DispatcherConfig) -> Result<Self, BackendError> {
        if config.queue_capacity == 0 || config.queue_capacity > MAX_QUEUE_CAPACITY {
            return Err(BackendError::BadParameters(
                "OneBot queue capacity is invalid".to_owned(),
            ));
        }
        Ok(Self::build(BTreeMap::new(), config))
    }

    fn build(accounts: BTreeMap<u64, Arc<dyn OneBotBackend>>, config: DispatcherConfig) -> Self {
        let (sender, mut receiver) = mpsc::channel::<QueuedAction>(config.queue_capacity);
        tokio::spawn(async move {
            while let Some(queued) = receiver.recv().await {
                let _result = queued.backend.call(queued.request).await;
            }
        });
        Self {
            accounts: RwLock::new(accounts),
            bound_self_id: config.bound_self_id,
            asynchronous: sender,
            rate_limited: Arc::new(Mutex::new(())),
            id_format: config.id_format,
        }
    }

    /// Registers a newly authenticated QQ account without replacing another runtime.
    ///
    /// # Errors
    ///
    /// Returns an error for a zero or duplicate QQ identifier or a poisoned registry.
    pub fn register(
        &self,
        self_id: u64,
        backend: Arc<dyn OneBotBackend>,
    ) -> Result<(), BackendError> {
        let mut accounts = self
            .accounts
            .write()
            .map_err(|_error| BackendError::AccountUnavailable)?;
        if self_id == 0 || accounts.contains_key(&self_id) {
            return Err(BackendError::BadParameters(
                "OneBot account registration is invalid".to_owned(),
            ));
        }
        accounts.insert(self_id, backend);
        Ok(())
    }

    /// Removes an account only when it is currently registered.
    #[must_use]
    pub fn unregister(&self, self_id: u64) -> bool {
        self.accounts
            .write()
            .ok()
            .and_then(|mut accounts| accounts.remove(&self_id))
            .is_some()
    }

    /// Returns the number of currently addressable QQ accounts.
    #[must_use]
    pub fn account_count(&self) -> usize {
        self.accounts.read().map_or(0, |accounts| accounts.len())
    }

    /// Executes or enqueues one validated action.
    #[must_use]
    pub async fn dispatch(&self, request: ActionRequest) -> ActionResponse {
        if request.action() == ".handle_quick_operation" {
            return self.dispatch_quick_operation(request).await;
        }
        self.dispatch_regular(request).await
    }

    async fn dispatch_quick_operation(&self, request: ActionRequest) -> ActionResponse {
        let echo = request.echo_owned();
        let actions = match expand_quick_operation(&request) {
            Ok(actions) => actions,
            Err(error) => return ActionResponse::backend_failure(echo, &error),
        };
        for action in actions {
            let response = self.dispatch_regular(action).await;
            if !matches!(response.retcode(), 0 | 1) {
                return ActionResponse::from_nested_failure(response, echo);
            }
        }
        ActionResponse::success(serde_json::Value::Null, echo)
    }

    async fn dispatch_regular(&self, request: ActionRequest) -> ActionResponse {
        let echo = request.echo_owned();
        let Some(backend) = self.select(request.self_id()) else {
            return if self.account_count() > 1 && self.bound_self_id.is_none() {
                ActionResponse::account_required(echo)
            } else {
                ActionResponse::backend_failure(echo, &BackendError::AccountUnavailable)
            };
        };
        match request.mode() {
            ActionMode::Synchronous => match backend.call(request.into_backend()).await {
                Ok(mut data) => {
                    self.id_format.project_data(&mut data);
                    ActionResponse::success(data, echo)
                }
                Err(error) => ActionResponse::backend_failure(echo, &error),
            },
            ActionMode::Asynchronous => {
                let queued = QueuedAction {
                    backend,
                    request: request.into_backend(),
                };
                match self.asynchronous.try_send(queued) {
                    Ok(()) => ActionResponse::asynchronous(echo),
                    Err(_error) => ActionResponse::backend_failure(echo, &BackendError::Overloaded),
                }
            }
            ActionMode::RateLimited => {
                let _guard = self.rate_limited.lock().await;
                match backend.call(request.into_backend()).await {
                    Ok(mut data) => {
                        self.id_format.project_data(&mut data);
                        ActionResponse::success(data, echo)
                    }
                    Err(error) => ActionResponse::backend_failure(echo, &error),
                }
            }
        }
    }

    fn select(&self, requested: Option<u64>) -> Option<Arc<dyn OneBotBackend>> {
        let accounts = self.accounts.read().ok()?;
        let self_id = self.bound_self_id.or(requested).or_else(|| {
            (accounts.len() == 1)
                .then(|| accounts.first_key_value().map(|(id, _)| *id))
                .flatten()
        })?;
        accounts.get(&self_id).cloned()
    }
}

impl core::fmt::Debug for OneBotDispatcher {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("OneBotDispatcher")
            .field("account_count", &self.account_count())
            .field("bound_self_id", &self.bound_self_id)
            .finish_non_exhaustive()
    }
}
