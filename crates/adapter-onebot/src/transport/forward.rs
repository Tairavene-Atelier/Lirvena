use core::future::Future;
use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use async_stream::stream;
use axum::body::{Body, Bytes};
use axum::extract::{DefaultBodyLimit, Path, State, WebSocketUpgrade};
use axum::http::{HeaderMap, StatusCode, Uri, header};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde_json::{Map, Value};
use tokio::net::TcpListener;

use crate::{ActionRequest, ActionResponse, OneBotDispatcher};

use super::OneBotEventBus;
use super::auth::{AccessToken, Authorization};
use super::ws::{self, WsCapabilities};

const MAX_BODY_BYTES: usize = 4 * 1_024 * 1_024;

/// Forward HTTP, WebSocket and SSE server configuration.
#[derive(Clone, Debug)]
pub struct ForwardServerConfig {
    /// Listen address. Port zero is accepted for tests and local discovery.
    pub listen: SocketAddr,
    /// Optional standard `Bearer` access token.
    pub access_token: Option<Vec<u8>>,
    /// Maximum HTTP action body bytes.
    pub max_body_bytes: usize,
}

/// Bound forward `OneBot` server.
pub struct OneBotForwardServer {
    listener: TcpListener,
    router: Router,
}

#[derive(Clone)]
struct ServerState {
    dispatcher: Arc<OneBotDispatcher>,
    events: Arc<OneBotEventBus>,
    access_token: Option<Arc<AccessToken>>,
}

impl OneBotForwardServer {
    /// Binds a forward server without starting request processing.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid body/token configuration or a listen failure.
    pub async fn bind(
        config: ForwardServerConfig,
        dispatcher: Arc<OneBotDispatcher>,
        events: Arc<OneBotEventBus>,
    ) -> Result<Self, OneBotForwardServerError> {
        if config.max_body_bytes == 0 || config.max_body_bytes > MAX_BODY_BYTES {
            return Err(OneBotForwardServerError::InvalidConfiguration);
        }
        let access_token = AccessToken::new(config.access_token)
            .map_err(|()| OneBotForwardServerError::InvalidConfiguration)?
            .map(Arc::new);
        let state = ServerState {
            dispatcher,
            events,
            access_token,
        };
        let router = Router::new()
            .route("/", get(ws_combined).post(canonical_action))
            .route("/api", get(ws_api))
            .route("/api/", get(ws_api))
            .route("/event", get(ws_event))
            .route("/event/", get(ws_event))
            .route("/ws", get(ws_combined))
            .route("/ws/", get(ws_combined))
            .route("/events", get(sse_events))
            .route("/{action}", get(http_get_action).post(http_post_action))
            .layer(DefaultBodyLimit::max(config.max_body_bytes))
            .with_state(state);
        let listener = TcpListener::bind(config.listen)
            .await
            .map_err(OneBotForwardServerError::Bind)?;
        Ok(Self { listener, router })
    }

    /// Returns the effective listen address.
    ///
    /// # Errors
    ///
    /// Returns the operating-system socket error when the address is unavailable.
    pub fn local_addr(&self) -> Result<SocketAddr, std::io::Error> {
        self.listener.local_addr()
    }

    /// Serves until graceful shutdown resolves.
    ///
    /// # Errors
    ///
    /// Returns a server I/O error.
    pub async fn serve(
        self,
        shutdown: impl Future<Output = ()> + Send + 'static,
    ) -> Result<(), OneBotForwardServerError> {
        axum::serve(self.listener, self.router)
            .with_graceful_shutdown(shutdown)
            .await
            .map_err(OneBotForwardServerError::Serve)
    }
}

async fn http_get_action(
    State(state): State<ServerState>,
    Path(action): Path<String>,
    headers: HeaderMap,
    uri: Uri,
) -> Response {
    if let Some(response) = authorize(&state, &headers, &uri) {
        return response;
    }
    let params = query_params(&uri);
    dispatch_http(&state, ActionRequest::from_http(&action, params)).await
}

async fn http_post_action(
    State(state): State<ServerState>,
    Path(action): Path<String>,
    headers: HeaderMap,
    uri: Uri,
    body: Bytes,
) -> Response {
    if let Some(response) = authorize(&state, &headers, &uri) {
        return response;
    }
    let params = match body_params(&headers, &body) {
        Ok(params) => params,
        Err(status) => return status.into_response(),
    };
    dispatch_http(&state, ActionRequest::from_http(&action, params)).await
}

async fn canonical_action(
    State(state): State<ServerState>,
    headers: HeaderMap,
    uri: Uri,
    body: Bytes,
) -> Response {
    if let Some(response) = authorize(&state, &headers, &uri) {
        return response;
    }
    if !is_json(&headers) {
        return StatusCode::NOT_ACCEPTABLE.into_response();
    }
    let request = serde_json::from_slice::<Value>(&body)
        .map_err(|_error| Box::new(ActionResponse::bad_request(None, "request JSON is invalid")))
        .and_then(ActionRequest::from_json);
    dispatch_http(&state, request).await
}

async fn dispatch_http(
    state: &ServerState,
    request: Result<ActionRequest, Box<ActionResponse>>,
) -> Response {
    let response = match request {
        Ok(request) => state.dispatcher.dispatch(request).await,
        Err(response) => *response,
    };
    let status = match response.retcode() {
        1400 => StatusCode::BAD_REQUEST,
        1404 => StatusCode::NOT_FOUND,
        _ => StatusCode::OK,
    };
    (status, Json(response)).into_response()
}

async fn ws_api(
    State(state): State<ServerState>,
    headers: HeaderMap,
    uri: Uri,
    upgrade: WebSocketUpgrade,
) -> Response {
    upgrade_ws(state, &headers, &uri, upgrade, true, false)
}

async fn ws_event(
    State(state): State<ServerState>,
    headers: HeaderMap,
    uri: Uri,
    upgrade: WebSocketUpgrade,
) -> Response {
    upgrade_ws(state, &headers, &uri, upgrade, false, true)
}

async fn ws_combined(
    State(state): State<ServerState>,
    headers: HeaderMap,
    uri: Uri,
    upgrade: WebSocketUpgrade,
) -> Response {
    upgrade_ws(state, &headers, &uri, upgrade, true, true)
}

fn upgrade_ws(
    state: ServerState,
    headers: &HeaderMap,
    uri: &Uri,
    upgrade: WebSocketUpgrade,
    actions: bool,
    events: bool,
) -> Response {
    if let Some(response) = authorize(&state, headers, uri) {
        return response;
    }
    upgrade
        .on_upgrade(move |socket| {
            ws::run(
                socket,
                state.dispatcher,
                state.events,
                WsCapabilities { actions, events },
            )
        })
        .into_response()
}

async fn sse_events(State(state): State<ServerState>, headers: HeaderMap, uri: Uri) -> Response {
    if let Some(response) = authorize(&state, &headers, &uri) {
        return response;
    }
    let mut subscription = state.events.subscribe();
    let output = stream! {
        loop {
            match subscription.recv().await {
                Ok(value) => match Event::default().json_data(value) {
                    Ok(event) => yield Ok::<Event, core::convert::Infallible>(event),
                    Err(_error) => break,
                },
                Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                    let value = serde_json::json!({"type": "lagged", "skipped": skipped});
                    match Event::default().event("lirvena_transport").json_data(value) {
                        Ok(event) => yield Ok(event),
                        Err(_error) => break,
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    };
    Sse::new(output)
        .keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
        .into_response()
}

fn authorize(state: &ServerState, headers: &HeaderMap, uri: &Uri) -> Option<Response> {
    let token = state.access_token.as_ref()?;
    match token.authorize(headers, uri) {
        Authorization::Allowed => None,
        Authorization::Missing => Some(StatusCode::UNAUTHORIZED.into_response()),
        Authorization::Denied => Some(StatusCode::FORBIDDEN.into_response()),
    }
}

fn query_params(uri: &Uri) -> Map<String, Value> {
    let parsed = uri
        .query()
        .and_then(|query| serde_urlencoded::from_str::<BTreeMap<String, String>>(query).ok())
        .unwrap_or_default();
    parsed
        .into_iter()
        .filter(|(name, _value)| name != "access_token")
        .map(|(name, value)| (name, Value::String(value)))
        .collect()
}

fn body_params(headers: &HeaderMap, body: &[u8]) -> Result<Map<String, Value>, StatusCode> {
    if is_json(headers) {
        return match serde_json::from_slice::<Value>(body) {
            Ok(Value::Object(params)) => Ok(params),
            Ok(_) | Err(_) => Err(StatusCode::BAD_REQUEST),
        };
    }
    if content_type(headers, "application/x-www-form-urlencoded") {
        return serde_urlencoded::from_bytes::<BTreeMap<String, String>>(body)
            .map(|params| {
                params
                    .into_iter()
                    .map(|(name, value)| (name, Value::String(value)))
                    .collect()
            })
            .map_err(|_error| StatusCode::BAD_REQUEST);
    }
    Err(StatusCode::NOT_ACCEPTABLE)
}

fn is_json(headers: &HeaderMap) -> bool {
    content_type(headers, "application/json")
}

fn content_type(headers: &HeaderMap, expected: &str) -> bool {
    headers
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .is_some_and(|value| value.trim().eq_ignore_ascii_case(expected))
}

/// Forward server construction or runtime error.
#[derive(Debug)]
pub enum OneBotForwardServerError {
    /// Configuration exceeded a closed bound.
    InvalidConfiguration,
    /// Listen socket creation failed.
    Bind(std::io::Error),
    /// HTTP server failed.
    Serve(std::io::Error),
}

impl core::fmt::Display for OneBotForwardServerError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidConfiguration => {
                formatter.write_str("OneBot server configuration is invalid")
            }
            Self::Bind(error) => write!(formatter, "OneBot server bind failed: {error}"),
            Self::Serve(error) => write!(formatter, "OneBot server failed: {error}"),
        }
    }
}

impl std::error::Error for OneBotForwardServerError {}

impl IntoResponse for OneBotForwardServerError {
    fn into_response(self) -> Response {
        (StatusCode::INTERNAL_SERVER_ERROR, Body::empty()).into_response()
    }
}
