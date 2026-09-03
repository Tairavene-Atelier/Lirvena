//! `OneBot` action routing contract tests.

use std::sync::Arc;

use adapter_onebot::{
    ActionRequest, BackendCall, BackendError, DispatcherConfig, IdFormat, OneBotBackend,
    OneBotDispatcher,
};
use serde_json::{Value, json};

struct EchoBackend;

impl OneBotBackend for EchoBackend {
    fn call(&self, request: ActionRequest) -> BackendCall<'_> {
        Box::pin(async move {
            if request.action() == "extension.anything" {
                Ok(json!({"routed": request.action()}))
            } else if request.action() == "get_friend_list" {
                Ok(
                    json!([{"user_id": 42, "nested": {"group_id": "7", "invitor_id": 8}, "file_id": "9"}]),
                )
            } else {
                Err(BackendError::ActionNotFound)
            }
        })
    }
}

#[tokio::test]
async fn configured_id_format_projects_nested_standard_identifiers() -> Result<(), BackendError> {
    let request = ActionRequest::from_json(json!({"action": "get_friend_list"}))
        .map_err(|_| BackendError::Failed("request rejected".to_owned()))?;
    let response = dispatcher(&[10001])?.dispatch(request).await;
    let encoded =
        serde_json::to_value(response).map_err(|error| BackendError::Failed(error.to_string()))?;
    assert_eq!(encoded["data"][0]["user_id"], json!("42"));
    assert_eq!(encoded["data"][0]["nested"]["group_id"], json!("7"));
    assert_eq!(encoded["data"][0]["nested"]["invitor_id"], json!("8"));
    assert_eq!(encoded["data"][0]["file_id"], json!("9"));
    Ok(())
}

fn dispatcher(accounts: &[u64]) -> Result<OneBotDispatcher, BackendError> {
    OneBotDispatcher::new(
        accounts
            .iter()
            .map(|id| (*id, Arc::new(EchoBackend) as Arc<dyn OneBotBackend>)),
        DispatcherConfig {
            bound_self_id: None,
            queue_capacity: 4,
            id_format: IdFormat::String,
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
