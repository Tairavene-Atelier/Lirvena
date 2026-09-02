use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket};
use futures_util::{SinkExt, StreamExt};
use serde_json::Value;

use crate::{ActionRequest, ActionResponse, OneBotDispatcher};

use super::OneBotEventBus;

#[derive(Clone, Copy)]
pub(super) struct WsCapabilities {
    pub(super) actions: bool,
    pub(super) events: bool,
}

pub(super) async fn run(
    socket: WebSocket,
    dispatcher: Arc<OneBotDispatcher>,
    events: Arc<OneBotEventBus>,
    capabilities: WsCapabilities,
) {
    let (mut outgoing, mut incoming) = socket.split();
    let mut subscription = events.subscribe();
    loop {
        tokio::select! {
            message = incoming.next() => {
                let Some(Ok(message)) = message else { break; };
                match message {
                    Message::Text(text) if capabilities.actions => {
                        let response = parse_and_dispatch(&dispatcher, &text).await;
                        let Ok(encoded) = serde_json::to_string(&response) else { break; };
                        if outgoing.send(Message::Text(encoded.into())).await.is_err() { break; }
                    }
                    Message::Close(_) => break,
                    Message::Ping(payload) => {
                        if outgoing.send(Message::Pong(payload)).await.is_err() { break; }
                    }
                    Message::Text(_) | Message::Binary(_) | Message::Pong(_) => {}
                }
            }
            event = subscription.recv(), if capabilities.events => {
                match event {
                    Ok(event) => {
                        let Ok(encoded) = serde_json::to_string(&event) else { break; };
                        if outgoing.send(Message::Text(encoded.into())).await.is_err() { break; }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                        let event = serde_json::json!({
                            "post_type": "meta_event",
                            "meta_event_type": "lirvena_transport",
                            "sub_type": "lagged",
                            "skipped": skipped
                        });
                        let Ok(encoded) = serde_json::to_string(&event) else { break; };
                        if outgoing.send(Message::Text(encoded.into())).await.is_err() { break; }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
        }
    }
}

pub(super) async fn parse_and_dispatch(
    dispatcher: &OneBotDispatcher,
    encoded: &str,
) -> ActionResponse {
    match serde_json::from_str::<Value>(encoded) {
        Ok(value) => match ActionRequest::from_json(value) {
            Ok(request) => dispatcher.dispatch(request).await,
            Err(response) => *response,
        },
        Err(_error) => ActionResponse::bad_request(None, "request JSON is invalid"),
    }
}
