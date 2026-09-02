//! Account action channel contract tests.

use account_api::{AccountActionError, AccountActionRequest, account_action_channel};
use serde_json::{Map, json};

#[tokio::test]
async fn request_is_completed_once_by_single_consumer() -> Result<(), AccountActionError> {
    let (handle, mut receiver) = account_action_channel(2)?;
    let task = tokio::spawn(async move {
        let pending = receiver
            .receive()
            .await
            .ok_or(AccountActionError::AccountUnavailable)?;
        assert_eq!(pending.request().action(), "extension.anything");
        pending.complete(Ok(json!({"accepted": true})));
        Ok::<(), AccountActionError>(())
    });
    let response = handle
        .execute(AccountActionRequest::new(
            "extension.anything".to_owned(),
            Map::new(),
        )?)
        .await?;
    assert_eq!(response, json!({"accepted": true}));
    task.await
        .map_err(|_error| AccountActionError::QqFailure)??;
    Ok(())
}

#[tokio::test]
async fn dropped_pending_action_fails_honestly() -> Result<(), AccountActionError> {
    let (handle, mut receiver) = account_action_channel(1)?;
    let task = tokio::spawn(async move {
        let _pending = receiver.receive().await;
    });
    let result = handle
        .execute(AccountActionRequest::new("unknown".to_owned(), Map::new())?)
        .await;
    task.await.map_err(|_error| AccountActionError::QqFailure)?;
    assert_eq!(result, Err(AccountActionError::AccountUnavailable));
    Ok(())
}
