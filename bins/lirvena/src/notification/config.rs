use std::env;
use std::io;
use std::path::{Path, PathBuf};

use notify_runtime::{
    BarkAdapter, BarkConfig, BarkLevel, DestinationId, NotificationAdapter,
    NotificationRuntimeConfig, SmtpAdapter, SmtpConfig, SmtpSecurity, WebhookAdapter,
    WebhookConfig,
};
use sha2::{Digest, Sha256};
use zeroize::{Zeroize, Zeroizing};

const QUEUE_CAPACITY: usize = 64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AdapterSelection {
    All,
    Bark,
    Webhook,
    Smtp,
}

impl AdapterSelection {
    pub(crate) fn parse(value: &str) -> Result<Self, io::Error> {
        match value {
            "all" => Ok(Self::All),
            "bark" => Ok(Self::Bark),
            "webhook" => Ok(Self::Webhook),
            "smtp" => Ok(Self::Smtp),
            _ => Err(invalid("notification adapter selection")),
        }
    }

    const fn includes(self, candidate: Self) -> bool {
        matches!(self, Self::All) || self as u8 == candidate as u8
    }
}

pub(super) struct PreparedNotifications {
    pub runtime: NotificationRuntimeConfig,
    pub adapter_count: usize,
}

pub(super) fn from_environment(
    state_directory: &Path,
    selection: AdapterSelection,
    allow_empty: bool,
) -> Result<Option<PreparedNotifications>, io::Error> {
    let mut adapters = Vec::new();
    if selection.includes(AdapterSelection::Bark)
        && let Some(adapter) = bark_from_environment()?
    {
        adapters.push(adapter);
    }
    if selection.includes(AdapterSelection::Webhook)
        && let Some(adapter) = webhook_from_environment()?
    {
        adapters.push(adapter);
    }
    if selection.includes(AdapterSelection::Smtp)
        && let Some(adapter) = smtp_from_environment()?
    {
        adapters.push(adapter);
    }
    if adapters.is_empty() {
        return if allow_empty {
            Ok(None)
        } else {
            Err(invalid("selected notification adapter configuration"))
        };
    }
    let adapter_count = adapters.len();
    let runtime = NotificationRuntimeConfig::new(
        state_directory.join("notifications"),
        adapters,
        QUEUE_CAPACITY,
    )
    .map_err(|_| invalid("notification runtime configuration"))?;
    Ok(Some(PreparedNotifications {
        runtime,
        adapter_count,
    }))
}

fn bark_from_environment() -> Result<Option<NotificationAdapter>, io::Error> {
    let Some(server) = optional_env("LIRVENA_NOTIFY_BARK_SERVER") else {
        return Ok(None);
    };
    let key = required_secret_text("LIRVENA_NOTIFY_BARK_KEY_PATH")?;
    let group = optional_env("LIRVENA_NOTIFY_BARK_GROUP");
    let level = optional_env("LIRVENA_NOTIFY_BARK_LEVEL")
        .map(|value| parse_bark_level(&value))
        .transpose()?;
    let url = optional_env("LIRVENA_NOTIFY_BARK_URL");
    let ciphertext = optional_secret_text("LIRVENA_NOTIFY_BARK_CIPHERTEXT_PATH")?;
    let id = destination_id("bark", &server);
    let config = BarkConfig::new(id, &server, key, group, level, url, ciphertext)
        .map_err(|_| invalid("Bark notification configuration"))?;
    BarkAdapter::new(config)
        .map(NotificationAdapter::Bark)
        .map(Some)
        .map_err(|_| invalid("Bark notification configuration"))
}

fn webhook_from_environment() -> Result<Option<NotificationAdapter>, io::Error> {
    let Some(endpoint) = optional_env("LIRVENA_NOTIFY_WEBHOOK_URL") else {
        return Ok(None);
    };
    let headers = optional_env("LIRVENA_NOTIFY_WEBHOOK_HEADERS_PATH")
        .map(PathBuf::from)
        .map(|path| parse_headers(&path))
        .transpose()?
        .unwrap_or_default();
    let hmac = optional_secret_bytes("LIRVENA_NOTIFY_WEBHOOK_HMAC_PATH")?;
    let id = destination_id("webhook", &endpoint);
    let config = WebhookConfig::new(id, &endpoint, headers, hmac)
        .map_err(|_| invalid("Webhook notification configuration"))?;
    WebhookAdapter::new(config)
        .map(NotificationAdapter::Webhook)
        .map(Some)
        .map_err(|_| invalid("Webhook notification configuration"))
}

fn smtp_from_environment() -> Result<Option<NotificationAdapter>, io::Error> {
    let Some(host) = optional_env("LIRVENA_NOTIFY_SMTP_HOST") else {
        return Ok(None);
    };
    let port = required_env("LIRVENA_NOTIFY_SMTP_PORT")?
        .parse::<u16>()
        .map_err(|_| invalid("LIRVENA_NOTIFY_SMTP_PORT"))?;
    let security = match required_env("LIRVENA_NOTIFY_SMTP_SECURITY")?.as_str() {
        "starttls" => SmtpSecurity::StartTls,
        "implicit_tls" => SmtpSecurity::ImplicitTls,
        _ => return Err(invalid("LIRVENA_NOTIFY_SMTP_SECURITY")),
    };
    let username = optional_secret_text("LIRVENA_NOTIFY_SMTP_USERNAME_PATH")?;
    let password = optional_secret_text("LIRVENA_NOTIFY_SMTP_PASSWORD_PATH")?;
    let from = required_env("LIRVENA_NOTIFY_SMTP_FROM")?;
    let to = required_env("LIRVENA_NOTIFY_SMTP_TO")?;
    let identity = format!("{host}:{port}:{from}:{to}");
    let config = SmtpConfig::new(
        destination_id("smtp", &identity),
        host,
        port,
        security,
        username,
        password,
        from,
        to,
    )
    .map_err(|_| invalid("SMTP notification configuration"))?;
    SmtpAdapter::new(&config)
        .map(NotificationAdapter::Smtp)
        .map(Some)
        .map_err(|_| invalid("SMTP notification configuration"))
}

fn destination_id(kind: &str, identity: &str) -> DestinationId {
    let digest = Sha256::digest([kind.as_bytes(), b"\0", identity.as_bytes()].concat());
    let mut bytes = [0_u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    DestinationId::from_bytes(bytes)
}

fn parse_bark_level(value: &str) -> Result<BarkLevel, io::Error> {
    match value {
        "critical" => Ok(BarkLevel::Critical),
        "active" => Ok(BarkLevel::Active),
        "time_sensitive" => Ok(BarkLevel::TimeSensitive),
        "passive" => Ok(BarkLevel::Passive),
        _ => Err(invalid("LIRVENA_NOTIFY_BARK_LEVEL")),
    }
}

fn parse_headers(path: &Path) -> Result<Vec<(String, Zeroizing<String>)>, io::Error> {
    let mut bytes = Zeroizing::new(
        local_state::read_private_file(path)
            .map_err(|_| invalid("LIRVENA_NOTIFY_WEBHOOK_HEADERS_PATH"))?,
    );
    let text =
        std::str::from_utf8(&bytes).map_err(|_| invalid("LIRVENA_NOTIFY_WEBHOOK_HEADERS_PATH"))?;
    let mut headers = Vec::new();
    for line in text.lines().filter(|line| !line.is_empty()) {
        let (name, value) = line
            .split_once(':')
            .ok_or_else(|| invalid("LIRVENA_NOTIFY_WEBHOOK_HEADERS_PATH"))?;
        headers.push((
            name.trim().to_owned(),
            Zeroizing::new(value.trim().to_owned()),
        ));
    }
    bytes.zeroize();
    Ok(headers)
}

fn optional_secret_text(name: &'static str) -> Result<Option<Zeroizing<String>>, io::Error> {
    optional_env(name)
        .map(PathBuf::from)
        .map(|path| secret_text(&path, name))
        .transpose()
}

fn required_secret_text(name: &'static str) -> Result<Zeroizing<String>, io::Error> {
    let path = PathBuf::from(required_env(name)?);
    secret_text(&path, name)
}

fn secret_text(path: &Path, name: &'static str) -> Result<Zeroizing<String>, io::Error> {
    let mut bytes =
        Zeroizing::new(local_state::read_private_file(path).map_err(|_| invalid(name))?);
    while matches!(bytes.last(), Some(b'\n' | b'\r')) {
        bytes.pop();
    }
    String::from_utf8(bytes.to_vec())
        .map(Zeroizing::new)
        .map_err(|_| invalid(name))
}

fn optional_secret_bytes(name: &'static str) -> Result<Option<Zeroizing<Vec<u8>>>, io::Error> {
    optional_env(name)
        .map(PathBuf::from)
        .map(|path| {
            local_state::read_private_file(&path)
                .map(Zeroizing::new)
                .map_err(|_| invalid(name))
        })
        .transpose()
}

fn required_env(name: &'static str) -> Result<String, io::Error> {
    optional_env(name).ok_or_else(|| invalid(name))
}

fn optional_env(name: &'static str) -> Option<String> {
    env::var(name).ok().filter(|value| !value.is_empty())
}

fn invalid(label: &'static str) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        format!("{label} is missing or invalid"),
    )
}
