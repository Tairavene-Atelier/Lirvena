use serde_json::{Map, Value};
use tokio::sync::{mpsc, oneshot};

const MAX_ACTION_BYTES: usize = 128;
const MAX_QUEUE_CAPACITY: usize = 1_024;

/// Adapter-neutral account action accepted by one QQ single-writer runtime.
#[derive(Clone, Debug, PartialEq)]
pub struct AccountActionRequest {
    action: String,
    params: Map<String, Value>,
}

impl AccountActionRequest {
    /// Creates a syntactically bounded action without imposing a business whitelist.
    ///
    /// # Errors
    ///
    /// Returns an error for an empty, excessive or control-character action name.
    pub fn new(action: String, params: Map<String, Value>) -> Result<Self, AccountActionError> {
        if action.is_empty()
            || action.len() > MAX_ACTION_BYTES
            || action.trim() != action
            || action.chars().any(char::is_control)
        {
            return Err(AccountActionError::BadParameters);
        }
        Ok(Self { action, params })
    }

    /// Returns the action name.
    #[must_use]
    pub fn action(&self) -> &str {
        &self.action
    }

    /// Returns the unmodified parameters.
    #[must_use]
    pub const fn params(&self) -> &Map<String, Value> {
        &self.params
    }
}

/// Cloneable bounded handle into one account's online actor.
#[derive(Clone)]
pub struct AccountActionHandle {
    sender: mpsc::Sender<Command>,
}

impl AccountActionHandle {
    /// Executes one action through the account's single writer.
    ///
    /// # Errors
    ///
    /// Returns an explicit action, queue, account or QQ failure.
    pub async fn execute(
        &self,
        request: AccountActionRequest,
    ) -> Result<Value, AccountActionError> {
        let (reply, receive) = oneshot::channel();
        self.sender
            .try_send(Command { request, reply })
            .map_err(|error| match error {
                mpsc::error::TrySendError::Full(_) => AccountActionError::Overloaded,
                mpsc::error::TrySendError::Closed(_) => AccountActionError::AccountUnavailable,
            })?;
        receive
            .await
            .map_err(|_error| AccountActionError::AccountUnavailable)?
    }

    /// Returns currently unused queue capacity.
    #[must_use]
    pub fn remaining_capacity(&self) -> usize {
        self.sender.capacity()
    }
}

impl core::fmt::Debug for AccountActionHandle {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("AccountActionHandle")
            .field("remaining_capacity", &self.remaining_capacity())
            .finish_non_exhaustive()
    }
}

/// Single-consumer action receiver owned by one QQ online actor.
pub struct AccountActionReceiver {
    receiver: mpsc::Receiver<Command>,
}

impl AccountActionReceiver {
    /// Receives the next queued action and its completion guard.
    pub async fn receive(&mut self) -> Option<PendingAccountAction> {
        self.receiver
            .recv()
            .await
            .map(|command| PendingAccountAction {
                request: command.request,
                reply: Some(command.reply),
            })
    }
}

impl core::fmt::Debug for AccountActionReceiver {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("AccountActionReceiver")
            .finish_non_exhaustive()
    }
}

/// One pending action that must receive an honest completion.
pub struct PendingAccountAction {
    request: AccountActionRequest,
    reply: Option<oneshot::Sender<Result<Value, AccountActionError>>>,
}

impl PendingAccountAction {
    /// Returns the request.
    #[must_use]
    pub const fn request(&self) -> &AccountActionRequest {
        &self.request
    }

    /// Completes the action exactly once.
    pub fn complete(mut self, result: Result<Value, AccountActionError>) {
        if let Some(reply) = self.reply.take() {
            let _ignored = reply.send(result);
        }
    }
}

impl core::fmt::Debug for PendingAccountAction {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("PendingAccountAction")
            .field("request", &self.request)
            .finish_non_exhaustive()
    }
}

impl Drop for PendingAccountAction {
    fn drop(&mut self) {
        if let Some(reply) = self.reply.take() {
            let _ignored = reply.send(Err(AccountActionError::AccountUnavailable));
        }
    }
}

struct Command {
    request: AccountActionRequest,
    reply: oneshot::Sender<Result<Value, AccountActionError>>,
}

/// Creates one bounded single-consumer account action channel.
///
/// # Errors
///
/// Returns an error for zero or excessive capacity.
pub fn account_action_channel(
    capacity: usize,
) -> Result<(AccountActionHandle, AccountActionReceiver), AccountActionError> {
    if capacity == 0 || capacity > MAX_QUEUE_CAPACITY {
        return Err(AccountActionError::BadParameters);
    }
    let (sender, receiver) = mpsc::channel(capacity);
    Ok((
        AccountActionHandle { sender },
        AccountActionReceiver { receiver },
    ))
}

/// Adapter-neutral action failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AccountActionError {
    /// No implementation owns this action name.
    ActionNotFound,
    /// Current QQ evidence does not support this action.
    Unsupported,
    /// Parameters are invalid.
    BadParameters,
    /// Account is not online.
    AccountUnavailable,
    /// Bounded action queue is full.
    Overloaded,
    /// QQ rejected or failed the operation.
    QqFailure,
}

impl core::fmt::Display for AccountActionError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::ActionNotFound => "account action is not implemented",
            Self::Unsupported => "account action is unsupported",
            Self::BadParameters => "account action parameters are invalid",
            Self::AccountUnavailable => "account is unavailable",
            Self::Overloaded => "account action queue is full",
            Self::QqFailure => "QQ action failed",
        })
    }
}

impl std::error::Error for AccountActionError {}
