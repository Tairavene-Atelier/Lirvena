//! Reverse `OneBot` WebSocket transport contract tests.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use adapter_onebot::{
    ActionRequest, BackendCall, DispatcherConfig, IdFormat, OneBotBackend, OneBotDispatcher,
    OneBotEventBus, ReverseWebSocket, ReverseWebSocketConfig,
};
use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use tokio::net::TcpListener;
use tokio::sync::watch;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::handshake::server::{Request, Response};

struct EchoBackend;

impl OneBotBackend for EchoBackend {
    fn call(&self, request: ActionRequest) -> BackendCall<'_> {
        Box::pin(async move { Ok(json!({"action": request.action()})) })
    }
}

#[tokio::test]
async fn reverse_websocket_authenticates_routes_and_filters_accounts()
-> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let dispatcher = Arc::new(OneBotDispatcher::new(
        [(10_001, Arc::new(EchoBackend) as Arc<dyn OneBotBackend>)],
        DispatcherConfig {
            bound_self_id: None,
            queue_capacity: 8,
            id_format: IdFormat::String,
        },
    )?);
    let events = Arc::new(OneBotEventBus::new(8)?);
    let reverse = ReverseWebSocket::new(ReverseWebSocketConfig {
        url: format!("ws://{address}/onebot"),
        access_token: Some(b"secret".to_vec().into()),
        self_id: 10_001,
        reconnect_delay: Duration::from_millis(100),
    })?;
    let (shutdown, receiver) = watch::channel(false);
    let task_dispatcher = dispatcher.clone();
    let task_events = events.clone();
    let task = tokio::spawn(async move {
        reverse.run(task_dispatcher, task_events, receiver).await;
    });

    let (stream, _peer) = listener.accept().await?;
    let captured = Arc::new(Mutex::new(None));
    let capture = captured.clone();
    #[allow(clippy::result_large_err)]
    let mut socket = tokio_tungstenite::accept_hdr_async(
        stream,
        move |request: &Request, response: Response| {
            let headers = (
                request
                    .headers()
                    .get("authorization")
                    .and_then(|value| value.to_str().ok())
                    .map(str::to_owned),
                request
                    .headers()
                    .get("x-self-id")
                    .and_then(|value| value.to_str().ok())
                    .map(str::to_owned),
                request
                    .headers()
                    .get("x-client-role")
                    .and_then(|value| value.to_str().ok())
                    .map(str::to_owned),
            );
            if let Ok(mut target) = capture.lock() {
                *target = Some(headers);
            }
            Ok(response)
        },
    )
    .await?;
    assert_eq!(
        captured.lock().ok().and_then(|value| value.clone()),
        Some((
            Some("Bearer secret".to_owned()),
            Some("10001".to_owned()),
            Some("Universal".to_owned()),
        ))
    );

    socket
        .send(Message::Text(
            json!({"action": "extension.echo", "self_id": 10001, "echo": 7})
                .to_string()
                .into(),
        ))
        .await?;
    let response = receive_json(&mut socket).await?;
    assert_eq!(response["status"], json!("ok"));
    assert_eq!(response["echo"], json!(7));

    let _other = events.publish(json!({"self_id": "10002", "post_type": "notice"}));
    let _own = events.publish(json!({"self_id": "10001", "post_type": "notice"}));
    let event = receive_json(&mut socket).await?;
    assert_eq!(event["self_id"], json!("10001"));

    let _changed = shutdown.send(true);
    tokio::time::timeout(Duration::from_secs(2), task).await??;
    Ok(())
}

async fn receive_json(
    socket: &mut tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let message = tokio::time::timeout(Duration::from_secs(2), socket.next())
        .await?
        .ok_or("reverse WebSocket closed")??;
    let Message::Text(text) = message else {
        return Err("reverse WebSocket returned non-text data".into());
    };
    Ok(serde_json::from_str(&text)?)
}
