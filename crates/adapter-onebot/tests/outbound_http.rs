//! HTTP event reporting and quick-operation contract tests.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use adapter_onebot::{
    ActionRequest, BackendCall, BackendError, DispatcherConfig, HttpEventReporter,
    HttpEventReporterConfig, IdFormat, OneBotBackend, OneBotDispatcher,
};
use axum::Router;
use axum::body::Bytes;
use axum::http::HeaderMap;
use axum::routing::post;
use serde_json::{Value, json};
use tokio::sync::{mpsc, oneshot};
use zeroize::Zeroizing;

struct RecordingBackend {
    calls: mpsc::UnboundedSender<(String, Value)>,
}

impl OneBotBackend for RecordingBackend {
    fn call(&self, request: ActionRequest) -> BackendCall<'_> {
        let action = request.action().to_owned();
        let params = Value::Object(request.params().clone());
        let calls = self.calls.clone();
        Box::pin(async move {
            calls
                .send((action, params))
                .map_err(|_error| BackendError::AccountUnavailable)?;
            Ok(Value::Null)
        })
    }
}

#[tokio::test]
async fn reporter_signs_identity_and_routes_quick_operation()
-> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (observed_send, mut observed_receive) = mpsc::unbounded_channel();
    let app = Router::new().route(
        "/events",
        post(move |headers: HeaderMap, body: Bytes| {
            let observed_send = observed_send.clone();
            async move {
                let signature = headers
                    .get("X-Signature")
                    .and_then(|value| value.to_str().ok())
                    .map(str::to_owned);
                let self_id = headers
                    .get("X-Self-ID")
                    .and_then(|value| value.to_str().ok())
                    .map(str::to_owned);
                let _result = observed_send.send((signature, self_id, body));
                axum::Json(json!({"reply": "hello"}))
            }
        }),
    );
    let listener =
        tokio::net::TcpListener::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0)).await?;
    let address = listener.local_addr()?;
    let (shutdown, shutdown_receive) = oneshot::channel();
    tokio::spawn(async move {
        let _result = axum::serve(listener, app)
            .with_graceful_shutdown(async move {
                let _result = shutdown_receive.await;
            })
            .await;
    });

    let (calls_send, mut calls_receive) = mpsc::unbounded_channel();
    let dispatcher = OneBotDispatcher::new(
        [(
            10_001,
            Arc::new(RecordingBackend { calls: calls_send }) as Arc<dyn OneBotBackend>,
        )],
        DispatcherConfig {
            bound_self_id: None,
            queue_capacity: 8,
            id_format: IdFormat::String,
        },
    )?;
    let reporter = HttpEventReporter::new(HttpEventReporterConfig {
        url: format!("http://{address}/events"),
        secret: Some(Zeroizing::new(b"test-secret".to_vec())),
        timeout: Duration::from_secs(5),
    })?;
    let event = json!({
        "self_id": "10001",
        "post_type": "message",
        "message_type": "private",
        "message_id": 7,
        "user_id": 20002
    });
    let response = reporter.report_and_handle(&event, &dispatcher).await?;
    assert_eq!(
        response
            .as_ref()
            .map(adapter_onebot::ActionResponse::retcode),
        Some(0)
    );

    let (signature, self_id, body) = observed_receive
        .recv()
        .await
        .ok_or("report was not observed")?;
    assert_eq!(self_id.as_deref(), Some("10001"));
    assert!(
        signature
            .as_deref()
            .is_some_and(|value| value.starts_with("sha1="))
    );
    assert_eq!(serde_json::from_slice::<Value>(&body)?, event);
    let (action, params) = calls_receive
        .recv()
        .await
        .ok_or("quick operation was not dispatched")?;
    assert_eq!(action, "send_private_msg");
    assert_eq!(params["user_id"], 20_002);
    assert_eq!(params["message"], "hello");
    let _result = shutdown.send(());
    Ok(())
}
