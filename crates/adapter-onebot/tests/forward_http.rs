//! Forward HTTP transport contract tests.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;

use adapter_onebot::{
    ActionRequest, BackendCall, BackendError, DispatcherConfig, ForwardServerConfig, IdFormat,
    OneBotBackend, OneBotDispatcher, OneBotEventBus, OneBotForwardServer,
};
use serde_json::{Value, json};
use tokio::sync::oneshot;

struct EchoBackend;

impl OneBotBackend for EchoBackend {
    fn call(&self, request: ActionRequest) -> BackendCall<'_> {
        Box::pin(async move { Ok(json!({"action": request.action(), "params": request.params()})) })
    }
}

async fn start(
    token: Option<&str>,
) -> Result<(SocketAddr, oneshot::Sender<()>), Box<dyn std::error::Error + Send + Sync>> {
    if rustls::crypto::CryptoProvider::get_default().is_none() {
        let _result = rustls::crypto::ring::default_provider().install_default();
    }
    let dispatcher = Arc::new(OneBotDispatcher::new(
        [(10001, Arc::new(EchoBackend) as Arc<dyn OneBotBackend>)],
        DispatcherConfig {
            bound_self_id: None,
            queue_capacity: 8,
            id_format: IdFormat::String,
        },
    )?);
    let server = OneBotForwardServer::bind(
        ForwardServerConfig {
            listen: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0),
            access_token: token.map(|value| value.as_bytes().to_vec()),
            max_body_bytes: 64 * 1_024,
        },
        dispatcher,
        Arc::new(OneBotEventBus::new(8)?),
    )
    .await?;
    let address = server.local_addr()?;
    let (shutdown, receiver) = oneshot::channel();
    tokio::spawn(async move {
        let _result = server
            .serve(async move {
                let _result = receiver.await;
            })
            .await;
    });
    Ok((address, shutdown))
}

#[tokio::test]
async fn arbitrary_extension_routes_over_http()
-> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (address, shutdown) = start(None).await?;
    let response = reqwest::Client::new()
        .post(format!("http://{address}/extension.anything"))
        .json(&json!({"value": "kept"}))
        .send()
        .await?;
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    let value: Value = response.json().await?;
    assert_eq!(value["status"], "ok");
    assert_eq!(value["data"]["action"], "extension.anything");
    assert_eq!(value["data"]["params"]["value"], "kept");
    let _result = shutdown.send(());
    Ok(())
}

#[tokio::test]
async fn configured_token_distinguishes_missing_and_invalid()
-> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (address, shutdown) = start(Some("secret-token")).await?;
    let client = reqwest::Client::new();
    let missing = client
        .get(format!("http://{address}/get_status"))
        .send()
        .await?;
    assert_eq!(missing.status(), reqwest::StatusCode::UNAUTHORIZED);
    let denied = client
        .get(format!("http://{address}/get_status"))
        .bearer_auth("wrong-token")
        .send()
        .await?;
    assert_eq!(denied.status(), reqwest::StatusCode::FORBIDDEN);
    let _result = shutdown.send(());
    Ok(())
}

#[test]
fn backend_error_is_send_sync() {
    fn assert_bounds<T: Send + Sync>() {}
    assert_bounds::<BackendError>();
}
