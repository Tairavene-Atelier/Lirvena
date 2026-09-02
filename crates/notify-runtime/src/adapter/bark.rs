use core::time::Duration;

use reqwest::redirect::Policy;
use serde::Serialize;
use zeroize::Zeroizing;

use super::{AdapterError, ensure_crypto_provider, severity_name};
use crate::{DestinationId, NotificationEvent, Severity};

/// Bark interruption level from the V2 API.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BarkLevel {
    /// Critical alert.
    Critical,
    /// Immediate active alert.
    Active,
    /// Time-sensitive alert.
    TimeSensitive,
    /// Passive alert.
    Passive,
}

impl BarkLevel {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Critical => "critical",
            Self::Active => "active",
            Self::TimeSensitive => "timeSensitive",
            Self::Passive => "passive",
        }
    }
}

/// Validated Bark Server V2 destination configuration.
pub struct BarkConfig {
    destination_id: DestinationId,
    server_url: String,
    device_key: Zeroizing<String>,
    group: Option<String>,
    level: Option<BarkLevel>,
    url: Option<String>,
    ciphertext: Option<Zeroizing<String>>,
}

impl BarkConfig {
    /// Creates a Bark destination. `server_url` is the server root, not a device-key path.
    ///
    /// # Errors
    ///
    /// Returns an error for a non-HTTP URL, URL credentials, empty device key, or oversized
    /// optional field.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        destination_id: DestinationId,
        server_url: &str,
        device_key: Zeroizing<String>,
        group: Option<String>,
        level: Option<BarkLevel>,
        url: Option<String>,
        ciphertext: Option<Zeroizing<String>>,
    ) -> Result<Self, AdapterError> {
        let parsed = parse_server_url(server_url)?;
        validate_secret(&device_key, 256)?;
        validate_optional(group.as_deref(), 128)?;
        validate_optional(url.as_deref(), 2_048)?;
        if let Some(ciphertext) = ciphertext.as_ref() {
            validate_secret(ciphertext, 8_192)?;
        }
        Ok(Self {
            destination_id,
            server_url: parsed.into(),
            device_key,
            group,
            level,
            url,
            ciphertext,
        })
    }
}

impl core::fmt::Debug for BarkConfig {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("BarkConfig")
            .field("destination_id", &self.destination_id)
            .field("server_url", &self.server_url)
            .field("device_key", &"<redacted>")
            .field("group", &self.group)
            .field("level", &self.level)
            .field("url", &self.url)
            .field(
                "ciphertext",
                &self.ciphertext.as_ref().map(|_| "<redacted>"),
            )
            .finish()
    }
}

/// Native Bark Server V2 adapter with one reusable HTTP client.
pub struct BarkAdapter {
    destination_id: DestinationId,
    endpoint: reqwest::Url,
    device_key: Zeroizing<String>,
    group: Option<String>,
    level: Option<BarkLevel>,
    url: Option<String>,
    ciphertext: Option<Zeroizing<String>>,
    client: reqwest::Client,
}

impl BarkAdapter {
    /// Builds a no-redirect, no-proxy HTTP client for the destination.
    ///
    /// # Errors
    ///
    /// Returns an error when URL or TLS client construction fails.
    pub fn new(config: BarkConfig) -> Result<Self, AdapterError> {
        ensure_crypto_provider()?;
        let mut endpoint = reqwest::Url::parse(&config.server_url)
            .map_err(|_error| AdapterError::Configuration)?;
        let path = format!("{}/push", endpoint.path().trim_end_matches('/'));
        endpoint.set_path(&path);
        let client = reqwest::Client::builder()
            .redirect(Policy::none())
            .no_proxy()
            .timeout(Duration::from_secs(15))
            .build()
            .map_err(|_error| AdapterError::Configuration)?;
        Ok(Self {
            destination_id: config.destination_id,
            endpoint,
            device_key: config.device_key,
            group: config.group,
            level: config.level,
            url: config.url,
            ciphertext: config.ciphertext,
            client,
        })
    }

    #[must_use]
    pub(super) const fn destination_id(&self) -> DestinationId {
        self.destination_id
    }

    pub(super) async fn deliver(&self, event: &NotificationEvent) -> Result<(), AdapterError> {
        let title = format!("Lirvena · {}", severity_name(event.severity()));
        let body = format!(
            "{}\n{}",
            event.human_summary().as_str(),
            event.next_action().as_str()
        );
        let level = self
            .level
            .unwrap_or_else(|| default_level(event.severity()));
        let payload = BarkPayload {
            title: &title,
            body: &body,
            device_key: &self.device_key,
            group: self.group.as_deref(),
            level: Some(level.as_str()),
            url: self.url.as_deref(),
            ciphertext: self.ciphertext.as_ref().map(|value| value.as_str()),
        };
        let response = self
            .client
            .post(self.endpoint.clone())
            .json(&payload)
            .send()
            .await
            .map_err(|_error| AdapterError::Transport)?;
        if response.status().is_success() {
            Ok(())
        } else {
            Err(AdapterError::Rejected)
        }
    }
}

impl core::fmt::Debug for BarkAdapter {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("BarkAdapter")
            .field("destination_id", &self.destination_id)
            .field("endpoint", &self.endpoint)
            .finish_non_exhaustive()
    }
}

#[derive(Serialize)]
struct BarkPayload<'a> {
    title: &'a str,
    body: &'a str,
    device_key: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    group: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    level: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    url: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ciphertext: Option<&'a str>,
}

fn parse_server_url(value: &str) -> Result<reqwest::Url, AdapterError> {
    let parsed = reqwest::Url::parse(value).map_err(|_error| AdapterError::Configuration)?;
    if !matches!(parsed.scheme(), "http" | "https")
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        return Err(AdapterError::Configuration);
    }
    Ok(parsed)
}

fn validate_optional(value: Option<&str>, maximum: usize) -> Result<(), AdapterError> {
    if value.is_some_and(|value| {
        value.is_empty() || value.len() > maximum || value.chars().any(char::is_control)
    }) {
        Err(AdapterError::Configuration)
    } else {
        Ok(())
    }
}

fn validate_secret(value: &str, maximum: usize) -> Result<(), AdapterError> {
    if value.is_empty() || value.len() > maximum || value.chars().any(char::is_whitespace) {
        Err(AdapterError::Configuration)
    } else {
        Ok(())
    }
}

const fn default_level(severity: Severity) -> BarkLevel {
    match severity {
        Severity::Info => BarkLevel::Passive,
        Severity::Warning => BarkLevel::TimeSensitive,
        Severity::Critical => BarkLevel::Critical,
    }
}
