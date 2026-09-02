//! Bounded asynchronous QQ frame transport tests.

use core::time::Duration;

use qq_transport::{QqTransport, TransportConfig, TransportError};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

fn config() -> Result<TransportConfig, TransportError> {
    TransportConfig::new(Duration::from_secs(1), Duration::from_secs(1), 1_024)
}

#[tokio::test(flavor = "current_thread")]
async fn reads_and_writes_one_complete_frame() -> Result<(), Box<dyn std::error::Error>> {
    let (client, mut peer) = tokio::io::duplex(1_024);
    let mut transport = QqTransport::from_stream(client, config()?);
    let frame = [0, 0, 0, 7, 1, 2, 3];

    peer.write_all(&frame).await?;
    assert_eq!(transport.read_frame().await?, frame);
    transport.write_frame(&frame).await?;
    let mut received = [0_u8; 7];
    peer.read_exact(&mut received).await?;
    assert_eq!(received, frame);
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn rejects_contradictory_and_excessive_lengths() -> Result<(), Box<dyn std::error::Error>> {
    let (client, mut peer) = tokio::io::duplex(1_024);
    let mut transport = QqTransport::from_stream(client, config()?);
    assert_eq!(
        transport.write_frame(&[0, 0, 0, 8, 1]).await,
        Err(TransportError::InvalidFrame)
    );
    peer.write_all(&2_048_u32.to_be_bytes()).await?;
    assert_eq!(
        transport.read_frame().await,
        Err(TransportError::InvalidFrame)
    );
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn idle_timeout_preserves_a_partial_frame() -> Result<(), Box<dyn std::error::Error>> {
    let (client, mut peer) = tokio::io::duplex(1_024);
    let config = TransportConfig::new(Duration::from_secs(1), Duration::from_millis(10), 1_024)?;
    let mut transport = QqTransport::from_stream(client, config);
    let frame = [0, 0, 0, 7, 1, 2, 3];

    peer.write_all(&frame[..2]).await?;
    assert_eq!(transport.read_frame().await, Err(TransportError::Timeout));
    peer.write_all(&frame[2..]).await?;
    assert_eq!(transport.read_frame().await?, frame);
    Ok(())
}
