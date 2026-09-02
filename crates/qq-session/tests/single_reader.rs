//! Authenticated single-reader demultiplexing tests.

use core::time::Duration;

use qq_envelope::{QqTeaKey, SessionAuth, encrypt_qq_tea};
use qq_session::{AuthenticatedSession, SessionError};
use qq_transport::{QqTransport, TransportConfig};
use qq_wire::{LengthPrefix, WireWriter};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

#[tokio::test(flavor = "current_thread")]
async fn admitted_push_cannot_steal_a_correlated_response() -> TestResult {
    let key = QqTeaKey::new([9; 16]);
    let auth = SessionAuth::authenticated(42, b"tgt", b"d2", &key)?;
    let (client, mut peer) = tokio::io::duplex(64 * 1024);
    let transport = QqTransport::from_stream(client, config()?);
    let mut session = AuthenticatedSession::new(transport);
    let push = response_frame(42, &key, 99, "push.alpha", b"push")?;
    let response = response_frame(42, &key, 17, "request.alpha", b"response")?;
    let peer_task = tokio::spawn(async move {
        let mut request = [0_u8; 4];
        peer.read_exact(&mut request).await?;
        peer.write_all(&push).await?;
        peer.write_all(&response).await?;
        std::io::Result::Ok(())
    });

    let body = session
        .exchange(&auth, 17, "request.alpha", &[0, 0, 0, 4], |route| {
            route == "push.alpha"
        })
        .await?;
    assert_eq!(body, b"response");
    let queued = session.pop_push().ok_or("missing queued Push")?;
    assert_eq!(queued.command(), "push.alpha");
    assert_eq!(queued.payload(), b"push");
    peer_task.await??;
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn unknown_or_same_sequence_frame_fails_closed() -> TestResult {
    let key = QqTeaKey::new([7; 16]);
    let auth = SessionAuth::authenticated(42, b"tgt", b"d2", &key)?;
    let (client, mut peer) = tokio::io::duplex(64 * 1024);
    let mut session = AuthenticatedSession::new(QqTransport::from_stream(client, config()?));
    let unknown = response_frame(42, &key, 18, "push.unknown", b"x")?;
    let peer_task = tokio::spawn(async move {
        let mut request = [0_u8; 4];
        peer.read_exact(&mut request).await?;
        peer.write_all(&unknown).await?;
        std::io::Result::Ok(())
    });
    assert!(matches!(
        session
            .exchange(&auth, 17, "request.alpha", &[0, 0, 0, 4], |_| false)
            .await,
        Err(SessionError::Protocol)
    ));
    peer_task.await??;
    Ok(())
}

fn config() -> Result<TransportConfig, qq_transport::TransportError> {
    TransportConfig::new(Duration::from_secs(1), Duration::from_secs(1), 64 * 1024)
}

fn response_frame(
    uin: u32,
    key: &QqTeaKey,
    sequence: u32,
    command: &str,
    payload: &[u8],
) -> TestResult<Vec<u8>> {
    let mut header = WireWriter::new(64 * 1024);
    header.put_u32(sequence)?;
    header.put_u32(0)?;
    header.put_prefixed_bytes(LengthPrefix::U32Inclusive, b"")?;
    header.put_prefixed_bytes(LengthPrefix::U32Inclusive, command.as_bytes())?;
    header.put_prefixed_bytes(LengthPrefix::U32Inclusive, b"")?;
    header.put_u32(0)?;
    header.put_prefixed_bytes(LengthPrefix::U32Inclusive, b"")?;
    let mut sso = WireWriter::new(64 * 1024);
    sso.put_prefixed_bytes(LengthPrefix::U32Inclusive, &header.finish())?;
    sso.put_prefixed_bytes(LengthPrefix::U32Inclusive, payload)?;
    let encrypted = encrypt_qq_tea(&sso.finish(), key)?;

    let mut body = WireWriter::new(64 * 1024);
    body.put_u32(12)?;
    body.put_u8(1)?;
    body.put_u8(0)?;
    body.put_prefixed_bytes(LengthPrefix::U32Inclusive, uin.to_string().as_bytes())?;
    body.put_bytes(&encrypted)?;
    let mut frame = WireWriter::new(64 * 1024);
    frame.put_prefixed_bytes(LengthPrefix::U32Inclusive, &body.finish())?;
    Ok(frame.finish())
}
