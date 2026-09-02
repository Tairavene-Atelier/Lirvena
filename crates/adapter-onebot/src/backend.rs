use core::future::Future;
use core::pin::Pin;

use serde_json::Value;

use account_api::{AccountActionError, AccountActionHandle, AccountActionRequest};

use crate::ActionRequest;

/// Heap-erased asynchronous account backend call.
pub type BackendCall<'a> = Pin<Box<dyn Future<Output = Result<Value, BackendError>> + Send + 'a>>;

/// Account-facing `OneBot` action executor.
///
/// Implementations receive every syntactically valid action name. The adapter does not impose a
/// business whitelist; unsupported operations must be reported explicitly by the QQ backend.
pub trait OneBotBackend: Send + Sync {
    /// Executes one action for this authenticated QQ account.
    fn call(&self, request: ActionRequest) -> BackendCall<'_>;
}

/// Honest action failure projected into the `OneBot` response envelope.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BackendError {
    /// The implementation has no action with this name.
    ActionNotFound,
    /// The action exists but current QQ evidence or account capability cannot execute it.
    Unsupported,
    /// Parameters are invalid for the selected action.
    BadParameters(String),
    /// The account is not online or is protectively offline.
    AccountUnavailable,
    /// The bounded account queue is full.
    Overloaded,
    /// QQ rejected or failed the operation.
    Failed(String),
}

impl core::fmt::Display for BackendError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::ActionNotFound => formatter.write_str("action is not implemented"),
            Self::Unsupported => formatter.write_str("action is unavailable for this account"),
            Self::BadParameters(message) | Self::Failed(message) => formatter.write_str(message),
            Self::AccountUnavailable => formatter.write_str("account is unavailable"),
            Self::Overloaded => formatter.write_str("account action queue is full"),
        }
    }
}

impl std::error::Error for BackendError {}

/// Reusable bridge from `OneBot` into one account's bounded QQ action channel.
#[derive(Clone, Debug)]
pub struct AccountChannelBackend {
    handle: AccountActionHandle,
}

impl AccountChannelBackend {
    /// Creates a backend for one account actor.
    #[must_use]
    pub const fn new(handle: AccountActionHandle) -> Self {
        Self { handle }
    }
}

impl OneBotBackend for AccountChannelBackend {
    fn call(&self, request: ActionRequest) -> BackendCall<'_> {
        Box::pin(async move {
            let action =
                AccountActionRequest::new(request.action().to_owned(), request.params().clone())
                    .map_err(map_account_error)?;
            self.handle.execute(action).await.map_err(map_account_error)
        })
    }
}

fn map_account_error(error: AccountActionError) -> BackendError {
    match error {
        AccountActionError::ActionNotFound => BackendError::ActionNotFound,
        AccountActionError::Unsupported => BackendError::Unsupported,
        AccountActionError::BadParameters => {
            BackendError::BadParameters("account action parameters are invalid".to_owned())
        }
        AccountActionError::AccountUnavailable => BackendError::AccountUnavailable,
        AccountActionError::Overloaded => BackendError::Overloaded,
        AccountActionError::QqFailure => BackendError::Failed("QQ action failed".to_owned()),
    }
}
