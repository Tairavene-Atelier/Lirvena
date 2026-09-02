mod bark;
mod smtp;
mod webhook;

pub use bark::{BarkAdapter, BarkConfig, BarkLevel};
pub use smtp::{SmtpAdapter, SmtpConfig, SmtpSecurity};
pub use webhook::{WebhookAdapter, WebhookConfig};

use crate::{Delivery, DestinationId};

/// Redacted single-attempt adapter failure with a stable outbox error code.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdapterError {
    /// Adapter configuration or canonical rendering was invalid.
    Configuration,
    /// Network, timeout, or TLS transport failed.
    Transport,
    /// Remote endpoint returned a non-success result.
    Rejected,
}

impl AdapterError {
    /// Stable nonzero code safe to persist in the outbox.
    #[must_use]
    pub const fn code(self) -> u32 {
        match self {
            Self::Configuration => 1,
            Self::Transport => 2,
            Self::Rejected => 3,
        }
    }
}

impl core::fmt::Display for AdapterError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("notification adapter operation failed")
    }
}

impl std::error::Error for AdapterError {}

/// Closed set of native Lirvena notification adapters.
pub enum NotificationAdapter {
    /// Bark Server V2 `POST /push`.
    Bark(BarkAdapter),
    /// Canonical JSON `HTTP POST` with optional HMAC authentication.
    Webhook(WebhookAdapter),
    /// Certificate-verified `SMTP` over STARTTLS or implicit TLS.
    Smtp(SmtpAdapter),
}

impl NotificationAdapter {
    /// Returns the stable local destination identifier.
    #[must_use]
    pub const fn destination_id(&self) -> DestinationId {
        match self {
            Self::Bark(adapter) => adapter.destination_id(),
            Self::Webhook(adapter) => adapter.destination_id(),
            Self::Smtp(adapter) => adapter.destination_id(),
        }
    }

    /// Performs exactly one delivery attempt without retrying internally.
    ///
    /// # Errors
    ///
    /// Returns a stable redacted configuration, transport, or rejection error.
    pub async fn deliver(
        &self,
        delivery: &Delivery,
        delivered_at_ms: u64,
    ) -> Result<(), AdapterError> {
        if delivery.destination_id() != self.destination_id() {
            return Err(AdapterError::Configuration);
        }
        match self {
            Self::Bark(adapter) => adapter.deliver(delivery.event()).await,
            Self::Webhook(adapter) => adapter.deliver(delivery.event(), delivered_at_ms).await,
            Self::Smtp(adapter) => adapter.deliver(delivery.event()).await,
        }
    }
}

impl core::fmt::Debug for NotificationAdapter {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("NotificationAdapter")
            .field("destination_id", &self.destination_id())
            .finish_non_exhaustive()
    }
}

pub(super) const fn source_name(source: crate::EventSource) -> &'static str {
    match source {
        crate::EventSource::Ceylith => "ceylith",
        crate::EventSource::Qq => "qq",
        crate::EventSource::Account => "account",
        crate::EventSource::Lirvena => "lirvena",
    }
}

pub(super) const fn category_name(category: crate::EventCategory) -> &'static str {
    match category {
        crate::EventCategory::Authorization => "authorization",
        crate::EventCategory::Continuity => "continuity",
        crate::EventCategory::RiskControl => "risk_control",
        crate::EventCategory::Worker => "worker",
        crate::EventCategory::Recovery => "recovery",
    }
}

pub(super) const fn severity_name(severity: crate::Severity) -> &'static str {
    match severity {
        crate::Severity::Info => "info",
        crate::Severity::Warning => "warning",
        crate::Severity::Critical => "critical",
    }
}

pub(super) const fn state_name(state: crate::EventState) -> &'static str {
    match state {
        crate::EventState::Current => "current",
        crate::EventState::Expiring => "expiring",
        crate::EventState::Paused => "paused",
        crate::EventState::Revoked => "revoked",
        crate::EventState::Unavailable => "unavailable",
        crate::EventState::ProtectiveOffline => "protective_offline",
        crate::EventState::Recovering => "recovering",
        crate::EventState::Active => "active",
        crate::EventState::Stopped => "stopped",
        crate::EventState::Failed => "failed",
    }
}

pub(super) fn encode_hex(bytes: &[u8]) -> String {
    use core::fmt::Write;

    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ignored = write!(&mut output, "{byte:02x}");
    }
    output
}

pub(super) fn ensure_crypto_provider() -> Result<(), AdapterError> {
    if rustls::crypto::CryptoProvider::get_default().is_some() {
        return Ok(());
    }
    let _ignored = rustls::crypto::ring::default_provider().install_default();
    if rustls::crypto::CryptoProvider::get_default().is_some() {
        Ok(())
    } else {
        Err(AdapterError::Configuration)
    }
}
