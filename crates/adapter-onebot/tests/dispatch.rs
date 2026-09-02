//! `OneBot` action routing contract tests.

use std::sync::Arc;

use adapter_onebot::{
    ActionRequest, BackendCall, BackendError, DispatcherConfig, OneBotBackend, OneBotDispatcher,
};
use serde_json::{Value, json};

struct EchoBackend;

impl OneBotBackend for EchoBackend {
    fn call(&self, request: ActionRequest) -> BackendCall<'_> {
        Box::pin(async move {
            if request.action() == "extension.anything" {
                Ok(json!({"routed": request.action()}))
            } else {
                Err(BackendError::ActionNotFound)
            }
        })
    }
}

fn dispatcher(accounts: &[u64]) -> Result<OneBotDispatcher, BackendError> {
    OneBotDispatcher::new(
        accounts
            .iter()
            .map(|id| (*id, Arc::new(EchoBackend) as Arc<dyn OneBotBackend>)),
        DispatcherConfig {
            bound_self_id: None,
            queue_capacity: 4,
        },
    )
}

#[tokio::test]
async fn arbitrary_extension_reaches_backend_without_whitelist() -> Result<(), BackendError> {
    let request = ActionRequest::from_json(json!({
        "action": "extension.anything",
        "params": {},
        "echo": {"opaque": true}
    }))
    .map_err(|_| BackendError::Failed("request rejected".to_owned()))?;
    let response = dispatcher(&[10001])?.dispatch(request).await;
    let encoded =
        serde_json::to_value(response).map_err(|error| BackendError::Failed(error.to_string()))?;
    assert_eq!(encoded["status"], Value::String("ok".to_owned()));
    assert_eq!(encoded["echo"], json!({"opaque": true}));
    Ok(())
}

#[tokio::test]
async fn multi_account_requires_explicit_self_id() -> Result<(), BackendError> {
    let request = ActionRequest::from_json(json!({"action": "extension.anything"}))
        .map_err(|_| BackendError::Failed("request rejected".to_owned()))?;
    let response = dispatcher(&[10001, 10002])?.dispatch(request).await;
    assert_eq!(response.retcode(), 1400);
    Ok(())
}

#[tokio::test]
async fn unknown_action_never_returns_fake_success() -> Result<(), BackendError> {
    let request = ActionRequest::from_json(json!({"action": "unknown"}))
        .map_err(|_| BackendError::Failed("request rejected".to_owned()))?;
    let response = dispatcher(&[10001])?.dispatch(request).await;
    assert_eq!(response.retcode(), 1404);
    Ok(())
}
