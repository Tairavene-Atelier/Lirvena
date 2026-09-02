use core::time::Duration;

use lettre::message::{Mailbox, MultiPart};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};
use tokio::time::timeout;
use zeroize::Zeroizing;

use super::{
    AdapterError, category_name, encode_hex, ensure_crypto_provider, severity_name, source_name,
    state_name,
};
use crate::{DestinationId, NotificationEvent};

const DELIVERY_TIMEOUT: Duration = Duration::from_secs(20);

/// Encrypted SMTP transport mode. No plaintext mode exists.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SmtpSecurity {
    /// Require an authenticated STARTTLS upgrade before credentials or content.
    StartTls,
    /// Establish implicit TLS before SMTP negotiation.
    ImplicitTls,
}

/// Validated SMTP destination configuration.
pub struct SmtpConfig {
    destination_id: DestinationId,
    host: String,
    port: u16,
    security: SmtpSecurity,
    username: Option<Zeroizing<String>>,
    password: Option<Zeroizing<String>>,
    from: String,
    to: String,
}

impl SmtpConfig {
    /// Creates a certificate-verified SMTP configuration.
    ///
    /// Username and password must either both be present or both absent. No plaintext transport
    /// or opportunistic TLS mode can be configured.
    ///
    /// # Errors
    ///
    /// Returns an error for an empty host, zero port, partial credentials, or malformed mailbox.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        destination_id: DestinationId,
        host: String,
        port: u16,
        security: SmtpSecurity,
        username: Option<Zeroizing<String>>,
        password: Option<Zeroizing<String>>,
        from: String,
        to: String,
    ) -> Result<Self, AdapterError> {
        let credentials_match = username.is_some() == password.is_some();
        if host.is_empty()
            || host.len() > 253
            || host.chars().any(char::is_whitespace)
            || port == 0
            || !credentials_match
            || username.as_ref().is_some_and(|value| value.is_empty())
            || password.as_ref().is_some_and(|value| value.is_empty())
            || from.parse::<Mailbox>().is_err()
            || to.parse::<Mailbox>().is_err()
        {
            return Err(AdapterError::Configuration);
        }
        Ok(Self {
            destination_id,
            host,
            port,
            security,
            username,
            password,
            from,
            to,
        })
    }
}

impl core::fmt::Debug for SmtpConfig {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("SmtpConfig")
            .field("destination_id", &self.destination_id)
            .field("host", &self.host)
            .field("port", &self.port)
            .field("security", &self.security)
            .field("credentials", &self.username.as_ref().map(|_| "<redacted>"))
            .field("from", &"<redacted>")
            .field("to", &"<redacted>")
            .field("password", &self.password.as_ref().map(|_| "<redacted>"))
            .finish_non_exhaustive()
    }
}

/// SMTP adapter using fixed plain-text and HTML message templates.
pub struct SmtpAdapter {
    destination_id: DestinationId,
    from: Mailbox,
    to: Mailbox,
    transport: AsyncSmtpTransport<Tokio1Executor>,
}

impl SmtpAdapter {
    /// Builds an SMTP transport with required certificate-verified encryption.
    ///
    /// # Errors
    ///
    /// Returns an error when TLS, credentials, or mailbox construction fails.
    pub fn new(config: &SmtpConfig) -> Result<Self, AdapterError> {
        ensure_crypto_provider()?;
        let builder = match config.security {
            SmtpSecurity::StartTls => {
                AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&config.host)
            }
            SmtpSecurity::ImplicitTls => AsyncSmtpTransport::<Tokio1Executor>::relay(&config.host),
        }
        .map_err(|_error| AdapterError::Configuration)?
        .port(config.port)
        .timeout(Some(DELIVERY_TIMEOUT));
        let builder = match (config.username.as_ref(), config.password.as_ref()) {
            (Some(username), Some(password)) => {
                builder.credentials(Credentials::new(username.to_string(), password.to_string()))
            }
            (None, None) => builder,
            _ => return Err(AdapterError::Configuration),
        };
        Ok(Self {
            destination_id: config.destination_id,
            from: config
                .from
                .parse()
                .map_err(|_error| AdapterError::Configuration)?,
            to: config
                .to
                .parse()
                .map_err(|_error| AdapterError::Configuration)?,
            transport: builder.build(),
        })
    }

    #[must_use]
    pub(super) const fn destination_id(&self) -> DestinationId {
        self.destination_id
    }

    pub(super) async fn deliver(&self, event: &NotificationEvent) -> Result<(), AdapterError> {
        let message = self.message(event)?;
        let result = timeout(DELIVERY_TIMEOUT, self.transport.send(message))
            .await
            .map_err(|_elapsed| AdapterError::Transport)?;
        result
            .map(|_response| ())
            .map_err(|_error| AdapterError::Transport)
    }

    fn message(&self, event: &NotificationEvent) -> Result<Message, AdapterError> {
        let transition = event.transition();
        let event_id = encode_hex(event.event_id().as_bytes());
        let subject = format!(
            "[Lirvena] {} {}",
            severity_name(event.severity()),
            category_name(event.category())
        );
        let plain = format!(
            "{}\n\nSource: {}\nState: {} -> {}\nNext action: {}\nEvent: {}\n",
            event.human_summary().as_str(),
            source_name(event.source()),
            state_name(transition.previous()),
            state_name(transition.current()),
            event.next_action().as_str(),
            event_id
        );
        let summary = html_escape::encode_text(event.human_summary().as_str());
        let next_action = html_escape::encode_text(event.next_action().as_str());
        let html = format!(
            "<!doctype html><html><body><h1>Lirvena</h1><p>{summary}</p>\
             <dl><dt>Source</dt><dd>{}</dd><dt>State</dt><dd>{} &rarr; {}</dd></dl>\
             <p><strong>Next action:</strong> {next_action}</p><p>Event: {event_id}</p>\
             </body></html>",
            source_name(event.source()),
            state_name(transition.previous()),
            state_name(transition.current()),
        );
        Message::builder()
            .from(self.from.clone())
            .to(self.to.clone())
            .subject(subject)
            .multipart(MultiPart::alternative_plain_html(plain, html))
            .map_err(|_error| AdapterError::Configuration)
    }
}

impl core::fmt::Debug for SmtpAdapter {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("SmtpAdapter")
            .field("destination_id", &self.destination_id)
            .field("mailboxes", &"<redacted>")
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        DedupeKey, EventCategory, EventId, EventSource, EventState, NotificationEvent,
        NotificationText, Severity, StateTransition,
    };

    use super::{SmtpAdapter, SmtpConfig, SmtpSecurity};

    #[test]
    fn fixed_templates_escape_html() -> Result<(), Box<dyn std::error::Error>> {
        let config = SmtpConfig::new(
            crate::DestinationId::from_bytes([1; 16]),
            String::from("smtp.example.com"),
            587,
            SmtpSecurity::StartTls,
            None,
            None,
            String::from("Lirvena <from@example.com>"),
            String::from("to@example.com"),
        )?;
        let adapter = SmtpAdapter::new(&config)?;
        let event = NotificationEvent::new(
            EventId::from_bytes([2; 16]),
            1,
            EventSource::Qq,
            EventCategory::RiskControl,
            Severity::Critical,
            None,
            1,
            StateTransition::new(EventState::Active, EventState::ProtectiveOffline)?,
            NotificationText::new("<unsafe>")?,
            NotificationText::new("Review & recover")?,
            DedupeKey::from_bytes([3; 32]),
        )?;
        let formatted = adapter.message(&event)?.formatted();
        let formatted = String::from_utf8(formatted)?;
        assert!(formatted.contains("&lt;unsafe&gt;"));
        assert!(formatted.contains("Content-Type: text/html"));
        Ok(())
    }
}
