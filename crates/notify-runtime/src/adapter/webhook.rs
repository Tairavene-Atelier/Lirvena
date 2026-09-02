use core::time::Duration;

use hmac::{Hmac, Mac};
use reqwest::header::{
    CONNECTION, CONTENT_LENGTH, CONTENT_TYPE, HOST, HeaderMap, HeaderName, HeaderValue,
    TRANSFER_ENCODING,
};
use reqwest::redirect::Policy;
use serde::Serialize;
use sha2::Sha256;
use zeroize::Zeroizing;

use super::{
    AdapterError, category_name, encode_hex, ensure_crypto_provider, severity_name, source_name,
    state_name,
};
use crate::{DestinationId, NotificationEvent};

const EVENT_ID_HEADER: &str = "x-lirvena-event-id";
const TIMESTAMP_HEADER: &str = "x-lirvena-timestamp";
const SIGNATURE_HEADER: &str = "x-lirvena-signature";
const MAX_STATIC_HEADERS: usize = 16;

/// Validated canonical JSON webhook configuration.
pub struct WebhookConfig {
    destination_id: DestinationId,
    endpoint: reqwest::Url,
    headers: HeaderMap,
    hmac_secret: Option<Zeroizing<Vec<u8>>>,
}

impl WebhookConfig {
    /// Creates a webhook configuration with static headers and optional HMAC-SHA256.
    ///
    /// Reserved framing headers cannot be overridden. URL credentials, redirects, and fragments
    /// are rejected.
    ///
    /// # Errors
    ///
    /// Returns an error for an unsafe URL, invalid or duplicate header, oversized header set, or
    /// an empty HMAC secret.
    pub fn new(
        destination_id: DestinationId,
        endpoint: &str,
        static_headers: Vec<(String, Zeroizing<String>)>,
        hmac_secret: Option<Zeroizing<Vec<u8>>>,
    ) -> Result<Self, AdapterError> {
        let endpoint = parse_endpoint(endpoint)?;
        if static_headers.len() > MAX_STATIC_HEADERS
            || hmac_secret.as_ref().is_some_and(|secret| secret.is_empty())
        {
            return Err(AdapterError::Configuration);
        }
        let mut headers = HeaderMap::with_capacity(static_headers.len());
        for (name, value) in static_headers {
            let name = HeaderName::from_bytes(name.as_bytes())
                .map_err(|_error| AdapterError::Configuration)?;
            if is_reserved_header(&name) || headers.contains_key(&name) {
                return Err(AdapterError::Configuration);
            }
            let mut value =
                HeaderValue::from_str(&value).map_err(|_error| AdapterError::Configuration)?;
            value.set_sensitive(true);
            headers.insert(name, value);
        }
        Ok(Self {
            destination_id,
            endpoint,
            headers,
            hmac_secret,
        })
    }
}

impl core::fmt::Debug for WebhookConfig {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("WebhookConfig")
            .field("destination_id", &self.destination_id)
            .field("endpoint", &self.endpoint)
            .field("static_header_count", &self.headers.len())
            .field("hmac", &self.hmac_secret.as_ref().map(|_| "<redacted>"))
            .finish()
    }
}

/// Canonical webhook adapter with one reusable HTTP client.
pub struct WebhookAdapter {
    config: WebhookConfig,
    client: reqwest::Client,
}

impl WebhookAdapter {
    /// Builds a no-redirect, no-proxy client with certificate verification enabled.
    ///
    /// # Errors
    ///
    /// Returns an error when HTTP client construction fails.
    pub fn new(config: WebhookConfig) -> Result<Self, AdapterError> {
        ensure_crypto_provider()?;
        let client = reqwest::Client::builder()
            .redirect(Policy::none())
            .no_proxy()
            .timeout(Duration::from_secs(15))
            .build()
            .map_err(|_error| AdapterError::Configuration)?;
        Ok(Self { config, client })
    }

    #[must_use]
    pub(super) const fn destination_id(&self) -> DestinationId {
        self.config.destination_id
    }

    pub(super) async fn deliver(
        &self,
        event: &NotificationEvent,
        delivered_at_ms: u64,
    ) -> Result<(), AdapterError> {
        let body = canonical_json(event)?;
        let event_id = encode_hex(event.event_id().as_bytes());
        let timestamp = delivered_at_ms.to_string();
        let mut request = self
            .client
            .post(self.config.endpoint.clone())
            .headers(self.config.headers.clone())
            .header(CONTENT_TYPE, "application/json; charset=utf-8")
            .header(EVENT_ID_HEADER, event_id)
            .header(TIMESTAMP_HEADER, &timestamp)
            .body(body.clone());
        if let Some(secret) = self.config.hmac_secret.as_ref() {
            request = request.header(SIGNATURE_HEADER, signature(secret, &timestamp, &body)?);
        }
        let response = request
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

impl core::fmt::Debug for WebhookAdapter {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("WebhookAdapter")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

#[derive(Serialize)]
struct CanonicalEvent<'a> {
    event_id: String,
    occurred_at: u64,
    source: &'static str,
    category: &'static str,
    severity: &'static str,
    account_local_id: Option<String>,
    reason_code: u32,
    state_transition: CanonicalTransition,
    human_summary: &'a str,
    next_action: &'a str,
    dedupe_key: String,
}

#[derive(Serialize)]
struct CanonicalTransition {
    previous: &'static str,
    current: &'static str,
}

fn canonical_json(event: &NotificationEvent) -> Result<Vec<u8>, AdapterError> {
    let transition = event.transition();
    serde_json::to_vec(&CanonicalEvent {
        event_id: encode_hex(event.event_id().as_bytes()),
        occurred_at: event.occurred_at_ms(),
        source: source_name(event.source()),
        category: category_name(event.category()),
        severity: severity_name(event.severity()),
        account_local_id: event.account_local_id().map(|value| encode_hex(value)),
        reason_code: event.reason_code(),
        state_transition: CanonicalTransition {
            previous: state_name(transition.previous()),
            current: state_name(transition.current()),
        },
        human_summary: event.human_summary().as_str(),
        next_action: event.next_action().as_str(),
        dedupe_key: encode_hex(event.dedupe_key().as_bytes()),
    })
    .map_err(|_error| AdapterError::Configuration)
}

fn signature(secret: &[u8], timestamp: &str, body: &[u8]) -> Result<String, AdapterError> {
    let mut mac =
        Hmac::<Sha256>::new_from_slice(secret).map_err(|_error| AdapterError::Configuration)?;
    mac.update(timestamp.as_bytes());
    mac.update(b"\n");
    mac.update(body);
    Ok(format!(
        "sha256={}",
        encode_hex(&mac.finalize().into_bytes())
    ))
}

fn parse_endpoint(value: &str) -> Result<reqwest::Url, AdapterError> {
    let parsed = reqwest::Url::parse(value).map_err(|_error| AdapterError::Configuration)?;
    if !matches!(parsed.scheme(), "http" | "https")
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.fragment().is_some()
    {
        return Err(AdapterError::Configuration);
    }
    Ok(parsed)
}

fn is_reserved_header(name: &HeaderName) -> bool {
    matches!(
        name,
        &HOST | &CONTENT_LENGTH | &CONTENT_TYPE | &TRANSFER_ENCODING | &CONNECTION
    ) || matches!(
        name.as_str(),
        EVENT_ID_HEADER | TIMESTAMP_HEADER | SIGNATURE_HEADER
    )
}

#[cfg(test)]
mod tests {
    use crate::{
        DedupeKey, EventCategory, EventId, EventSource, EventState, NotificationEvent,
        NotificationText, Severity, StateTransition,
    };

    use super::{canonical_json, signature};

    #[test]
    fn canonical_body_and_signature_are_stable() -> Result<(), Box<dyn std::error::Error>> {
        let event = NotificationEvent::new(
            EventId::from_bytes([1; 16]),
            42,
            EventSource::Ceylith,
            EventCategory::Authorization,
            Severity::Critical,
            Some([2; 16]),
            7,
            StateTransition::new(EventState::Current, EventState::Revoked)?,
            NotificationText::new("Grant revoked")?,
            NotificationText::new("Review settings")?,
            DedupeKey::from_bytes([3; 32]),
        )?;
        let body = canonical_json(&event)?;
        assert_eq!(body, canonical_json(&event)?);
        let signed = signature(b"secret", "123", &body)?;
        assert_eq!(signed, signature(b"secret", "123", &body)?);
        assert!(signed.starts_with("sha256="));
        Ok(())
    }
}
