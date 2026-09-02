use core::time::Duration;

use ceylith_crypto::NoisePublicKey;
use ceylith_protocol::{HARD_MAX_OUTER_FRAME_LEN, RequestId, WireLimits, proto};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpStream, ToSocketAddrs};
use tokio::time::timeout;

use crate::{
    AccessToken, ClientConnection, ClientError, InstallationIdentity, PendingHandshake,
    RuntimeDescriptor,
};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const IO_TIMEOUT: Duration = Duration::from_secs(30);

/// Authenticated Ceylith client over the bounded raw TCP carrier.
pub struct CeylithTcpClient {
    stream: TcpStream,
    connection: ClientConnection,
}

impl CeylithTcpClient {
    /// Establishes TCP and completes the authenticated Ceylith handshake.
    ///
    /// # Errors
    ///
    /// Returns an error for carrier, identity, authentication or admission failure.
    pub async fn connect<A: ToSocketAddrs>(
        address: A,
        identity: &InstallationIdentity,
        server_static_key: NoisePublicKey,
        token: Option<&AccessToken>,
        runtime: &RuntimeDescriptor,
        requested_feature_bits: u64,
        limits: WireLimits,
    ) -> Result<Self, ClientError> {
        let mut stream = timeout(CONNECT_TIMEOUT, TcpStream::connect(address))
            .await
            .map_err(|_elapsed| ClientError::Carrier)??;
        stream.set_nodelay(true)?;
        let (pending, hello) = PendingHandshake::start(
            identity,
            server_static_key,
            token,
            runtime,
            requested_feature_bits,
            limits,
        )?;
        write_frame(&mut stream, &hello).await?;
        let welcome = read_frame(&mut stream).await?;
        let connection = pending.finish(&welcome)?;
        Ok(Self { stream, connection })
    }

    /// Returns the authenticated admission and secure-session state.
    #[must_use]
    pub const fn connection(&self) -> &ClientConnection {
        &self.connection
    }

    /// Seals, sends, receives and authenticates one request-response exchange.
    ///
    /// # Errors
    ///
    /// Returns an error for carrier, counter, request binding or protocol failure.
    pub async fn exchange(
        &mut self,
        request_id: RequestId,
        request: &proto::InnerFrame,
    ) -> Result<proto::InnerFrame, ClientError> {
        let encoded = self.connection.seal(request_id, request)?;
        write_frame(&mut self.stream, &encoded).await?;
        let encoded = read_frame(&mut self.stream).await?;
        let (response_id, response) = self.connection.open(&encoded)?;
        if response_id != request_id {
            return Err(ClientError::SessionBinding);
        }
        Ok(response)
    }
}

impl core::fmt::Debug for CeylithTcpClient {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("CeylithTcpClient")
            .field("connection", &self.connection)
            .finish_non_exhaustive()
    }
}

async fn read_frame(stream: &mut TcpStream) -> Result<Vec<u8>, ClientError> {
    let mut prefix = [0_u8; 4];
    timeout(IO_TIMEOUT, stream.read_exact(&mut prefix))
        .await
        .map_err(|_elapsed| ClientError::Carrier)??;
    let length = usize::try_from(u32::from_be_bytes(prefix)).map_err(|_| ClientError::Carrier)?;
    if length == 0 || length > HARD_MAX_OUTER_FRAME_LEN {
        return Err(ClientError::Carrier);
    }
    let mut frame = vec![0_u8; length];
    timeout(IO_TIMEOUT, stream.read_exact(&mut frame))
        .await
        .map_err(|_elapsed| ClientError::Carrier)??;
    Ok(frame)
}

async fn write_frame(stream: &mut TcpStream, frame: &[u8]) -> Result<(), ClientError> {
    let length = u32::try_from(frame.len()).map_err(|_| ClientError::Carrier)?;
    timeout(IO_TIMEOUT, async {
        stream.write_all(&length.to_be_bytes()).await?;
        stream.write_all(frame).await?;
        stream.flush().await
    })
    .await
    .map_err(|_elapsed| ClientError::Carrier)??;
    Ok(())
}
