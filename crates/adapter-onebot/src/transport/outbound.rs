use std::sync::Arc;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use hmac::{Hmac, Mac};
use serde_json::Value;
use sha1::Sha1;
use tokio::sync::watch;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::HeaderValue;
use tokio_tungstenite::tungstenite::protocol::Message;
use url::Url;
use zeroize::Zeroizing;

use crate::{ActionRequest, ActionResponse, OneBotDispatcher};

use super::OneBotEventBus;
use super::ws::parse_and_dispatch;

const MAX_EVENT_BYTES: usize = 4 * 1_024 * 1_024;
const MAX_TOKEN_BYTES: usize = 1_024;

/// Standard `OneBot` HTTP POST event reporter configuration.
#[derive(Clone)]
pub struct HttpEventReporterConfig {
    /// User-owned HTTP or HTTPS endpoint.
    pub url: String,
    /// Optional HMAC-SHA1 secret required by the `OneBot` 11 signature header.
    pub secret: Option<Zeroizing<Vec<u8>>>,
    /// Per-event request timeout.
    pub timeout: Duration,
}

impl core::fmt::Debug for HttpEventReporterConfig {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("HttpEventReporterConfig")
            .field("url", &self.url)
            .field("secret", &self.secret.as_ref().map(|_| "<redacted>"))
            .field("timeout", &self.timeout)
            .finish()
    }
}

/// One validated HTTP event reporter.
pub struct HttpEventReporter {
    client: reqwest::Client,
    url: Url,
    secret: Option<Zeroizing<Vec<u8>>>,
}

impl HttpEventReporter {
    /// Creates a no-redirect reporter with certificate verification enabled.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid URL, secret, timeout or TLS provider.
    pub fn new(config: HttpEventReporterConfig) -> Result<Self, OutboundTransportError> {
        ensure_tls_provider()?;
        let url =
            Url::parse(&config.url).map_err(|_error| OutboundTransportError::Configuration)?;
        if !matches!(url.scheme(), "http" | "https")
            || config.timeout.is_zero()
            || config.timeout > Duration::from_mins(2)
            || config
                .secret
                .as_ref()
                .is_some_and(|secret| secret.is_empty() || secret.len() > MAX_TOKEN_BYTES)
        {
            return Err(OutboundTransportError::Configuration);
        }
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(config.timeout)
            .build()
            .map_err(|_error| OutboundTransportError::Configuration)?;
        Ok(Self {
            client,
            url,
            secret: config.secret,
        })
    }

    /// Sends one canonical event and returns a non-empty quick-operation body when supplied.
    ///
    /// # Errors
    ///
    /// Returns an error for serialization, transport, non-success status or oversized response.
    pub async fn send_event(&self, event: &Value) -> Result<Option<Value>, OutboundTransportError> {
        let body = serde_json::to_vec(event).map_err(|_error| OutboundTransportError::Protocol)?;
        if body.len() > MAX_EVENT_BYTES {
            return Err(OutboundTransportError::Protocol);
        }
        let mut request = self
            .client
            .post(self.url.clone())
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(body.clone());
        if let Some(self_id) = event.get("self_id").and_then(Value::as_u64).or_else(|| {
            event
                .get("self_id")
                .and_then(Value::as_str)
                .and_then(|value| value.parse::<u64>().ok())
        }) {
            request = request.header("X-Self-ID", self_id);
        }
        if let Some(secret) = &self.secret {
            let mut mac = Hmac::<Sha1>::new_from_slice(secret)
                .map_err(|_error| OutboundTransportError::Configuration)?;
            mac.update(&body);
            request = request.header(
                "X-Signature",
                format!("sha1={}", encode_hex(&mac.finalize().into_bytes())),
            );
        }
        let response = request
            .send()
            .await
            .map_err(|_error| OutboundTransportError::Transport)?;
        if !response.status().is_success() {
            return Err(OutboundTransportError::Rejected);
        }
        let bytes = response
            .bytes()
            .await
            .map_err(|_error| OutboundTransportError::Transport)?;
        if bytes.is_empty() {
            return Ok(None);
        }
        if bytes.len() > MAX_EVENT_BYTES {
            return Err(OutboundTransportError::Protocol);
        }
        serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|_error| OutboundTransportError::Protocol)
    }

    /// Reports an event and executes a non-empty standard quick operation through the same
    /// account dispatcher.
    ///
    /// # Errors
    ///
    /// Returns an outbound transport error before dispatch. An invalid quick-operation shape is
    /// returned as an honest failed `OneBot` response.
    pub async fn report_and_handle(
        &self,
        event: &Value,
        dispatcher: &OneBotDispatcher,
    ) -> Result<Option<ActionResponse>, OutboundTransportError> {
        let Some(operation) = self.send_event(event).await? else {
            return Ok(None);
        };
        if operation.as_object().is_some_and(serde_json::Map::is_empty) {
            return Ok(None);
        }
        let request = ActionRequest::from_json(serde_json::json!({
            "action": ".handle_quick_operation",
            "params": {
                "context": event,
                "operation": operation,
                "self_id": event.get("self_id").cloned().unwrap_or(Value::Null)
            }
        }));
        Ok(Some(match request {
            Ok(request) => dispatcher.dispatch(request).await,
            Err(response) => *response,
        }))
    }
}

impl core::fmt::Debug for HttpEventReporter {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("HttpEventReporter")
            .field("url", &self.url)
            .field("secret", &self.secret.as_ref().map(|_| "<redacted>"))
            .finish_non_exhaustive()
    }
}

/// Reverse WebSocket universal-endpoint configuration.
#[derive(Clone)]
pub struct ReverseWebSocketConfig {
    /// User-owned `ws` or `wss` URL.
    pub url: String,
    /// Optional standard bearer token.
    pub access_token: Option<Zeroizing<Vec<u8>>>,
    /// QQ account exposed by this endpoint.
    pub self_id: u64,
    /// Delay before reconnecting a failed connection.
    pub reconnect_delay: Duration,
}

impl core::fmt::Debug for ReverseWebSocketConfig {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ReverseWebSocketConfig")
            .field("url", &self.url)
            .field(
                "access_token",
                &self.access_token.as_ref().map(|_| "<redacted>"),
            )
            .field("self_id", &self.self_id)
            .field("reconnect_delay", &self.reconnect_delay)
            .finish()
    }
}

/// Reconnecting standard reverse WebSocket client.
pub struct ReverseWebSocket {
    config: ReverseWebSocketConfig,
}

impl ReverseWebSocket {
    /// Validates a reverse WebSocket endpoint.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid URL, account, token or reconnect delay.
    pub fn new(config: ReverseWebSocketConfig) -> Result<Self, OutboundTransportError> {
        ensure_tls_provider()?;
        let url =
            Url::parse(&config.url).map_err(|_error| OutboundTransportError::Configuration)?;
        if !matches!(url.scheme(), "ws" | "wss")
            || config.self_id == 0
            || config.reconnect_delay < Duration::from_millis(100)
            || config.reconnect_delay > Duration::from_mins(5)
            || config.access_token.as_ref().is_some_and(|token| {
                token.is_empty() || token.len() > MAX_TOKEN_BYTES || token.contains(&0)
            })
        {
            return Err(OutboundTransportError::Configuration);
        }
        Ok(Self { config })
    }

    /// Reconnects and serves the universal reverse endpoint until shutdown becomes true.
    pub async fn run(
        &self,
        dispatcher: Arc<OneBotDispatcher>,
        events: Arc<OneBotEventBus>,
        mut shutdown: watch::Receiver<bool>,
    ) {
        while !*shutdown.borrow() {
            let result = self
                .run_connection(dispatcher.clone(), events.clone(), shutdown.clone())
                .await;
            if *shutdown.borrow() {
                break;
            }
            if result.is_err() {
                tokio::select! {
                    () = tokio::time::sleep(self.config.reconnect_delay) => {}
                    changed = shutdown.changed() => {
                        if changed.is_err() || *shutdown.borrow() { break; }
                    }
                }
            }
        }
    }

    async fn run_connection(
        &self,
        dispatcher: Arc<OneBotDispatcher>,
        events: Arc<OneBotEventBus>,
        mut shutdown: watch::Receiver<bool>,
    ) -> Result<(), OutboundTransportError> {
        let mut request = self
            .config
            .url
            .as_str()
            .into_client_request()
            .map_err(|_error| OutboundTransportError::Configuration)?;
        request.headers_mut().insert(
            "X-Self-ID",
            HeaderValue::from_str(&self.config.self_id.to_string())
                .map_err(|_error| OutboundTransportError::Configuration)?,
        );
        request
            .headers_mut()
            .insert("X-Client-Role", HeaderValue::from_static("Universal"));
        if let Some(token) = &self.config.access_token {
            let token = core::str::from_utf8(token)
                .map_err(|_error| OutboundTransportError::Configuration)?;
            request.headers_mut().insert(
                "Authorization",
                HeaderValue::from_str(&format!("Bearer {token}"))
                    .map_err(|_error| OutboundTransportError::Configuration)?,
            );
        }
        let (socket, _response) = tokio_tungstenite::connect_async(request)
            .await
            .map_err(|_error| OutboundTransportError::Transport)?;
        let (mut outgoing, mut incoming) = socket.split();
        let mut subscription = events.subscribe();
        loop {
            tokio::select! {
                incoming = incoming.next() => {
                    match incoming {
                        Some(Ok(Message::Text(text))) => {
                            let response = parse_and_dispatch(&dispatcher, &text).await;
                            let encoded = serde_json::to_string(&response)
                                .map_err(|_error| OutboundTransportError::Protocol)?;
                            outgoing.send(Message::Text(encoded.into())).await
                                .map_err(|_error| OutboundTransportError::Transport)?;
                        }
                        Some(Ok(Message::Ping(value))) => outgoing.send(Message::Pong(value)).await
                            .map_err(|_error| OutboundTransportError::Transport)?,
                        Some(Ok(Message::Close(_))) | None => return Ok(()),
                        Some(Ok(Message::Binary(_) | Message::Pong(_) | Message::Frame(_))) => {}
                        Some(Err(_error)) => return Err(OutboundTransportError::Transport),
                    }
                }
                event = subscription.recv() => match event {
                    Ok(event) => {
                        if !belongs_to_account(&event, self.config.self_id) {
                            continue;
                        }
                        let encoded = serde_json::to_string(&event)
                            .map_err(|_error| OutboundTransportError::Protocol)?;
                        outgoing.send(Message::Text(encoded.into())).await
                            .map_err(|_error| OutboundTransportError::Transport)?;
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                        return Err(OutboundTransportError::Lagged);
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => return Ok(()),
                },
                changed = shutdown.changed() => {
                    if changed.is_err() || *shutdown.borrow() {
                        let _result = outgoing.send(Message::Close(None)).await;
                        return Ok(());
                    }
                }
            }
        }
    }
}

fn belongs_to_account(event: &Value, self_id: u64) -> bool {
    event.get("self_id").is_some_and(|value| match value {
        Value::Number(number) => number.as_u64() == Some(self_id),
        Value::String(value) => value.parse::<u64>() == Ok(self_id),
        _ => false,
    })
}

impl core::fmt::Debug for ReverseWebSocket {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("ReverseWebSocket")
            .field(&self.config)
            .finish()
    }
}

fn ensure_tls_provider() -> Result<(), OutboundTransportError> {
    if rustls::crypto::CryptoProvider::get_default().is_none() {
        let _result = rustls::crypto::ring::default_provider().install_default();
    }
    rustls::crypto::CryptoProvider::get_default()
        .is_some()
        .then_some(())
        .ok_or(OutboundTransportError::Configuration)
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}

/// Outbound `OneBot` transport failure without secret contents.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutboundTransportError {
    /// Configuration violates a closed bound.
    Configuration,
    /// Network or TLS transport failed.
    Transport,
    /// Peer returned a non-success response.
    Rejected,
    /// JSON or message shape is invalid or oversized.
    Protocol,
    /// Event consumer fell behind the bounded bus.
    Lagged,
}

impl core::fmt::Display for OutboundTransportError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::Configuration => "OneBot outbound configuration is invalid",
            Self::Transport => "OneBot outbound transport failed",
            Self::Rejected => "OneBot HTTP event endpoint rejected the event",
            Self::Protocol => "OneBot outbound protocol data is invalid",
            Self::Lagged => "OneBot outbound event stream lagged",
        })
    }
}

impl std::error::Error for OutboundTransportError {}
