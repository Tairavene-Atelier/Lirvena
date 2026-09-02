use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::timeout;

use crate::{QqEndpoint, TransportConfig, TransportError};

/// Length-delimited QQ transport over an injected asynchronous stream.
#[derive(Debug)]
pub struct QqTransport<T> {
    stream: T,
    config: TransportConfig,
    read_prefix: [u8; 4],
    read_prefix_len: usize,
    read_frame: Vec<u8>,
    read_frame_len: usize,
}

impl QqTransport<TcpStream> {
    /// Connects to one compile-time allowlisted QQ endpoint.
    ///
    /// # Errors
    ///
    /// Returns an error for DNS/TCP failure or an elapsed connect deadline.
    pub async fn connect(
        endpoint: QqEndpoint,
        config: TransportConfig,
    ) -> Result<Self, TransportError> {
        let stream = timeout(
            config.connect_timeout,
            TcpStream::connect(endpoint.address()),
        )
        .await
        .map_err(|_elapsed| TransportError::Timeout)??;
        stream.set_nodelay(true)?;
        Ok(Self::from_stream(stream, config))
    }
}

impl<T> QqTransport<T> {
    /// Wraps an established asynchronous byte stream.
    #[must_use]
    pub const fn from_stream(stream: T, config: TransportConfig) -> Self {
        Self {
            stream,
            config,
            read_prefix: [0; 4],
            read_prefix_len: 0,
            read_frame: Vec::new(),
            read_frame_len: 0,
        }
    }

    /// Returns the inner stream for an orderly owner-controlled shutdown.
    #[must_use]
    pub fn into_inner(self) -> T {
        self.stream
    }
}

impl<T> QqTransport<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    /// Writes one already length-delimited complete frame.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid declared length, I/O failure or timeout.
    pub async fn write_frame(&mut self, frame: &[u8]) -> Result<(), TransportError> {
        validate_complete_frame(frame, self.config.maximum_frame_len)?;
        timeout(self.config.io_timeout, self.stream.write_all(frame))
            .await
            .map_err(|_elapsed| TransportError::Timeout)??;
        timeout(self.config.io_timeout, self.stream.flush())
            .await
            .map_err(|_elapsed| TransportError::Timeout)??;
        Ok(())
    }

    /// Reads one length-delimited complete frame.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid length, I/O failure or timeout.
    pub async fn read_frame(&mut self) -> Result<Vec<u8>, TransportError> {
        while self.read_prefix_len < self.read_prefix.len() {
            let count = timeout(
                self.config.io_timeout,
                self.stream
                    .read(&mut self.read_prefix[self.read_prefix_len..]),
            )
            .await
            .map_err(|_elapsed| TransportError::Timeout)??;
            if count == 0 {
                return Err(std::io::Error::from(std::io::ErrorKind::UnexpectedEof).into());
            }
            self.read_prefix_len += count;
        }
        if self.read_frame.is_empty() {
            let declared = usize::try_from(u32::from_be_bytes(self.read_prefix))
                .map_err(|_error| TransportError::InvalidFrame)?;
            if declared < self.read_prefix.len() || declared > self.config.maximum_frame_len {
                return Err(TransportError::InvalidFrame);
            }
            self.read_frame.resize(declared, 0);
            self.read_frame[..self.read_prefix.len()].copy_from_slice(&self.read_prefix);
            self.read_frame_len = self.read_prefix.len();
        }
        while self.read_frame_len < self.read_frame.len() {
            let count = timeout(
                self.config.io_timeout,
                self.stream
                    .read(&mut self.read_frame[self.read_frame_len..]),
            )
            .await
            .map_err(|_elapsed| TransportError::Timeout)??;
            if count == 0 {
                return Err(std::io::Error::from(std::io::ErrorKind::UnexpectedEof).into());
            }
            self.read_frame_len += count;
        }
        self.read_prefix_len = 0;
        self.read_frame_len = 0;
        Ok(std::mem::take(&mut self.read_frame))
    }
}

fn validate_complete_frame(frame: &[u8], maximum: usize) -> Result<(), TransportError> {
    let prefix: [u8; 4] = frame
        .get(..4)
        .ok_or(TransportError::InvalidFrame)?
        .try_into()
        .map_err(|_error| TransportError::InvalidFrame)?;
    let declared = usize::try_from(u32::from_be_bytes(prefix))
        .map_err(|_error| TransportError::InvalidFrame)?;
    if declared != frame.len() || declared > maximum {
        Err(TransportError::InvalidFrame)
    } else {
        Ok(())
    }
}
