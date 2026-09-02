use std::io;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::time::Duration;

use adapter_onebot::{HttpEventReporterConfig, ReverseWebSocketConfig};
use serde::Deserialize;
use zeroize::Zeroizing;

use super::read::{invalid_config, optional_secret_path};

const DEFAULT_BODY_BYTES: usize = 1_048_576;
const DEFAULT_EVENT_CAPACITY: usize = 1_024;
const MAX_OUTBOUND_ENDPOINTS: usize = 32;

/// Optional installation-wide `OneBot` endpoint configuration.
pub(crate) struct OneBotConfig {
    pub(crate) forward_listen: SocketAddr,
    pub(crate) access_token: Option<Zeroizing<Vec<u8>>>,
    pub(crate) max_body_bytes: usize,
    pub(crate) event_queue_capacity: usize,
    pub(crate) http_post: Vec<HttpEventReporterConfig>,
    pub(crate) reverse_websocket: Vec<ReverseWebSocketConfig>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct OneBotSection {
    forward: ForwardSection,
    #[serde(default = "default_event_capacity")]
    event_queue_capacity: usize,
    #[serde(default)]
    http_post: Vec<HttpPostSection>,
    #[serde(default)]
    reverse_websocket: Vec<ReverseWebSocketSection>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ForwardSection {
    listen: SocketAddr,
    access_token_path: Option<PathBuf>,
    #[serde(default = "default_body_bytes")]
    max_body_bytes: usize,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HttpPostSection {
    url: String,
    secret_path: Option<PathBuf>,
    #[serde(default = "default_http_timeout_seconds")]
    timeout_seconds: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReverseWebSocketSection {
    url: String,
    access_token_path: Option<PathBuf>,
    self_id: u64,
    #[serde(default = "default_reconnect_milliseconds")]
    reconnect_milliseconds: u64,
}

impl OneBotSection {
    pub(super) fn load(self, base: &Path) -> Result<OneBotConfig, io::Error> {
        if self.event_queue_capacity == 0
            || self.event_queue_capacity > 4_096
            || self.forward.max_body_bytes == 0
            || self.forward.max_body_bytes > 4 * 1_048_576
        {
            return Err(invalid_config("onebot bounds"));
        }
        if self.http_post.len() > MAX_OUTBOUND_ENDPOINTS
            || self.reverse_websocket.len() > MAX_OUTBOUND_ENDPOINTS
        {
            return Err(invalid_config("onebot outbound endpoint count"));
        }
        let token_path = self.forward.access_token_path.map(|path| {
            if path.is_absolute() {
                path
            } else {
                base.join(path)
            }
        });
        let http_post = self
            .http_post
            .into_iter()
            .map(|endpoint| endpoint.load(base))
            .collect::<Result<Vec<_>, _>>()?;
        let reverse_websocket = self
            .reverse_websocket
            .into_iter()
            .map(|endpoint| endpoint.load(base))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(OneBotConfig {
            forward_listen: self.forward.listen,
            access_token: optional_secret_path(token_path.as_deref(), "OneBot access token")?,
            max_body_bytes: self.forward.max_body_bytes,
            event_queue_capacity: self.event_queue_capacity,
            http_post,
            reverse_websocket,
        })
    }
}

impl HttpPostSection {
    fn load(self, base: &Path) -> Result<HttpEventReporterConfig, io::Error> {
        if self.timeout_seconds == 0 || self.timeout_seconds > 120 {
            return Err(invalid_config("onebot HTTP POST timeout"));
        }
        Ok(HttpEventReporterConfig {
            url: self.url,
            secret: optional_secret_path(
                self.secret_path
                    .as_deref()
                    .map(|path| resolve(base, path))
                    .as_deref(),
                "OneBot HTTP POST secret",
            )?,
            timeout: Duration::from_secs(self.timeout_seconds),
        })
    }
}

impl ReverseWebSocketSection {
    fn load(self, base: &Path) -> Result<ReverseWebSocketConfig, io::Error> {
        if self.reconnect_milliseconds < 100 || self.reconnect_milliseconds > 300_000 {
            return Err(invalid_config("onebot reverse WebSocket reconnect delay"));
        }
        Ok(ReverseWebSocketConfig {
            url: self.url,
            access_token: optional_secret_path(
                self.access_token_path
                    .as_deref()
                    .map(|path| resolve(base, path))
                    .as_deref(),
                "OneBot reverse WebSocket access token",
            )?,
            self_id: self.self_id,
            reconnect_delay: Duration::from_millis(self.reconnect_milliseconds),
        })
    }
}

fn resolve(base: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_owned()
    } else {
        base.join(path)
    }
}

const fn default_body_bytes() -> usize {
    DEFAULT_BODY_BYTES
}

const fn default_event_capacity() -> usize {
    DEFAULT_EVENT_CAPACITY
}

const fn default_http_timeout_seconds() -> u64 {
    15
}

const fn default_reconnect_milliseconds() -> u64 {
    5_000
}
