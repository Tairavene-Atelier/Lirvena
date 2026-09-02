//! Notification worker lifecycle and durable delivery tests.

use notify_runtime::{
    DedupeKey, DestinationId, EventCategory, EventId, EventSource, EventState, NotificationAdapter,
    NotificationEvent, NotificationRuntimeConfig, NotificationText, Severity, StateTransition,
    WebhookAdapter, WebhookConfig, spawn_notification_runtime,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

const NOW_MS: u64 = 2_000_000;

#[tokio::test]
async fn worker_persists_flushes_and_stops_cleanly() -> Result<(), Box<dyn std::error::Error>> {
    let temporary = tempfile::tempdir()?;
    let (endpoint, captured) = capture_one_request().await?;
    let destination = DestinationId::from_bytes([0x41; 16]);
    let adapter = NotificationAdapter::Webhook(WebhookAdapter::new(WebhookConfig::new(
        destination,
        &endpoint,
        Vec::new(),
        None,
    )?)?);
    let config = NotificationRuntimeConfig::new(temporary.path().join("notify"), vec![adapter], 4)?;
    let runtime = spawn_notification_runtime(config).await?;
    let handle = runtime.handle();

    assert_eq!(handle.enqueue(event()?, NOW_MS).await?, 1);
    let sweep = handle.flush(NOW_MS).await?;
    assert_eq!(sweep.attempted(), 1);
    assert_eq!(sweep.delivered(), 1);
    assert_eq!(sweep.failed(), 0);
    assert!(captured.await??.starts_with("POST / HTTP/1.1\r\n"));
    assert_eq!(handle.flush(NOW_MS + 1).await?.attempted(), 0);
    runtime.shutdown().await?;
    Ok(())
}

#[test]
fn runtime_configuration_rejects_missing_destinations() {
    let error = NotificationRuntimeConfig::new("notify".into(), Vec::new(), 4).err();
    assert_eq!(
        error,
        Some(notify_runtime::NotificationError::Configuration)
    );
}

#[tokio::test]
async fn nonblocking_enqueue_is_flushed_during_shutdown() -> Result<(), Box<dyn std::error::Error>>
{
    let temporary = tempfile::tempdir()?;
    let (endpoint, captured) = capture_one_request().await?;
    let destination = DestinationId::from_bytes([0x44; 16]);
    let adapter = NotificationAdapter::Webhook(WebhookAdapter::new(WebhookConfig::new(
        destination,
        &endpoint,
        Vec::new(),
        None,
    )?)?);
    let runtime = spawn_notification_runtime(NotificationRuntimeConfig::new(
        temporary.path().join("notify"),
        vec![adapter],
        1,
    )?)
    .await?;
    runtime.handle().try_enqueue(event()?, NOW_MS)?;
    runtime.shutdown().await?;
    assert!(captured.await??.starts_with("POST / HTTP/1.1\r\n"));
    Ok(())
}

fn event() -> Result<NotificationEvent, notify_runtime::NotificationError> {
    NotificationEvent::new(
        EventId::from_bytes([0x42; 16]),
        NOW_MS,
        EventSource::Lirvena,
        EventCategory::Worker,
        Severity::Warning,
        None,
        1,
        StateTransition::new(EventState::Failed, EventState::Recovering)?,
        NotificationText::new("Notification runtime test")?,
        NotificationText::new("No action is required")?,
        DedupeKey::from_bytes([0x43; 32]),
    )
}

async fn capture_one_request() -> Result<
    (
        String,
        tokio::task::JoinHandle<Result<String, std::io::Error>>,
    ),
    std::io::Error,
> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let task = tokio::spawn(async move {
        let (mut stream, _peer) = listener.accept().await?;
        let mut bytes = Vec::new();
        let mut buffer = [0_u8; 1_024];
        let expected = loop {
            let read = stream.read(&mut buffer).await?;
            if read == 0 {
                return Err(std::io::Error::from(std::io::ErrorKind::UnexpectedEof));
            }
            bytes.extend_from_slice(&buffer[..read]);
            if let Some(header_end) = find_header_end(&bytes) {
                let headers = String::from_utf8_lossy(&bytes[..header_end]);
                let content_length = headers
                    .lines()
                    .find_map(|line| {
                        line.to_ascii_lowercase()
                            .strip_prefix("content-length:")
                            .map(str::trim)
                            .and_then(|value| value.parse::<usize>().ok())
                    })
                    .ok_or_else(|| std::io::Error::from(std::io::ErrorKind::InvalidData))?;
                break header_end + 4 + content_length;
            }
        };
        while bytes.len() < expected {
            let read = stream.read(&mut buffer).await?;
            if read == 0 {
                break;
            }
            bytes.extend_from_slice(&buffer[..read]);
        }
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}")
            .await?;
        stream.shutdown().await?;
        String::from_utf8(bytes)
            .map_err(|_error| std::io::Error::from(std::io::ErrorKind::InvalidData))
    });
    Ok((format!("http://{address}"), task))
}

fn find_header_end(bytes: &[u8]) -> Option<usize> {
    bytes.windows(4).position(|window| window == b"\r\n\r\n")
}
