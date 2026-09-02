//! Loopback transport tests for Bark and canonical webhook delivery.

use notify_runtime::{
    BarkAdapter, BarkConfig, BarkLevel, DedupeKey, DestinationId, EventCategory, EventId,
    EventSource, EventState, NotificationAdapter, NotificationEvent, NotificationStore,
    NotificationText, Severity, StateTransition, WebhookAdapter, WebhookConfig,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use zeroize::Zeroizing;

const NOW_MS: u64 = 1_000_000;

#[tokio::test]
async fn bark_posts_v2_push_payload() -> Result<(), Box<dyn std::error::Error>> {
    let (endpoint, captured) = capture_one_request().await?;
    let destination = DestinationId::from_bytes([0x31; 16]);
    let config = BarkConfig::new(
        destination,
        &endpoint,
        Zeroizing::new(String::from("device-key")),
        Some(String::from("Lirvena")),
        Some(BarkLevel::TimeSensitive),
        Some(String::from("https://example.com/status")),
        None,
    )?;
    let adapter = NotificationAdapter::Bark(BarkAdapter::new(config)?);
    let delivery = delivery(destination, 1)?;
    adapter.deliver(&delivery, NOW_MS).await?;
    let request = captured.await??;
    assert!(request.starts_with("POST /push HTTP/1.1\r\n"));
    assert!(request.contains("\"device_key\":\"device-key\""));
    assert!(request.contains("\"group\":\"Lirvena\""));
    assert!(request.contains("\"level\":\"timeSensitive\""));
    Ok(())
}

#[tokio::test]
async fn webhook_posts_canonical_headers_and_hmac() -> Result<(), Box<dyn std::error::Error>> {
    let (endpoint, captured) = capture_one_request().await?;
    let destination = DestinationId::from_bytes([0x32; 16]);
    let config = WebhookConfig::new(
        destination,
        &endpoint,
        vec![(
            String::from("X-Static"),
            Zeroizing::new(String::from("configured")),
        )],
        Some(Zeroizing::new(b"hmac-secret".to_vec())),
    )?;
    let adapter = NotificationAdapter::Webhook(WebhookAdapter::new(config)?);
    let delivery = delivery(destination, 2)?;
    adapter.deliver(&delivery, NOW_MS).await?;
    let request = captured.await??;
    let lowercase = request.to_ascii_lowercase();
    assert!(request.starts_with("POST / HTTP/1.1\r\n"));
    assert!(lowercase.contains("x-static: configured\r\n"));
    assert!(lowercase.contains("x-lirvena-event-id: 02020202020202020202020202020202"));
    assert!(lowercase.contains("x-lirvena-timestamp: 1000000"));
    assert!(lowercase.contains("x-lirvena-signature: sha256="));
    assert!(request.contains("\"state_transition\":{"));
    Ok(())
}

fn delivery(
    destination: DestinationId,
    marker: u8,
) -> Result<notify_runtime::Delivery, Box<dyn std::error::Error>> {
    let temporary = tempfile::tempdir()?;
    let mut store = NotificationStore::open(&temporary.path().join("notify"))?;
    let event = NotificationEvent::new(
        EventId::from_bytes([marker; 16]),
        NOW_MS,
        EventSource::Ceylith,
        EventCategory::Authorization,
        Severity::Critical,
        None,
        1,
        StateTransition::new(EventState::Current, EventState::Revoked)?,
        NotificationText::new("Grant revoked")?,
        NotificationText::new("Review Lirvena settings")?,
        DedupeKey::from_bytes([marker; 32]),
    )?;
    store.enqueue(&event, &[destination], NOW_MS)?;
    store
        .due(NOW_MS, 1)?
        .into_iter()
        .next()
        .ok_or_else(|| std::io::Error::other("test delivery was not due").into())
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
                return Err(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "HTTP request ended before headers",
                ));
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
                    .ok_or_else(|| {
                        std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            "HTTP request omitted content length",
                        )
                    })?;
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
            .map_err(|_error| std::io::Error::new(std::io::ErrorKind::InvalidData, "non-UTF8 HTTP"))
    });
    Ok((format!("http://{address}"), task))
}

fn find_header_end(bytes: &[u8]) -> Option<usize> {
    bytes.windows(4).position(|window| window == b"\r\n\r\n")
}
