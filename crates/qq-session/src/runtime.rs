use std::collections::VecDeque;

use qq_envelope::{SessionAuth, SsoResponse, decode_session_frame};
use qq_transport::QqTransport;
use tokio::io::{AsyncRead, AsyncWrite};

use crate::SessionError;

const MAX_QUEUED_PUSHES: usize = 64;
const MAX_INTERLEAVED_PUSHES: usize = 64;

/// Owns the sole post-login read path for one authenticated transport generation.
#[derive(Debug)]
pub struct AuthenticatedSession<T> {
    transport: QqTransport<T>,
    pushes: VecDeque<SsoResponse>,
}

impl<T> AuthenticatedSession<T> {
    /// Adopts an established transport after successful credential exchange.
    #[must_use]
    pub const fn new(transport: QqTransport<T>) -> Self {
        Self {
            transport,
            pushes: VecDeque::new(),
        }
    }

    /// Returns one already authenticated and admitted queued Push.
    pub fn pop_push(&mut self) -> Option<SsoResponse> {
        self.pushes.pop_front()
    }

    /// Returns the transport when this generation is being discarded.
    #[must_use]
    pub fn into_transport(self) -> QqTransport<T> {
        self.transport
    }
}

impl<T> AuthenticatedSession<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    /// Writes one already encoded authenticated frame without opening another read path.
    ///
    /// # Errors
    ///
    /// Returns an error when the underlying transport rejects the frame.
    pub async fn send(&mut self, frame: &[u8]) -> Result<(), SessionError> {
        self.transport.write_frame(frame).await.map_err(Into::into)
    }

    /// Sends one request and keeps admitted asynchronous Push frames out of its response path.
    ///
    /// # Errors
    ///
    /// Returns an error for transport/envelope failure, remote rejection, an unknown Push,
    /// or a breached compiled queue bound.
    pub async fn exchange<F>(
        &mut self,
        auth: &SessionAuth<'_>,
        sequence: u32,
        command: &str,
        frame: &[u8],
        admits_push: F,
    ) -> Result<Vec<u8>, SessionError>
    where
        F: Fn(&str) -> bool,
    {
        self.transport.write_frame(frame).await?;
        for _ in 0..=MAX_INTERLEAVED_PUSHES {
            let inbound = self.read(auth).await?;
            if inbound.sequence() == sequence && inbound.command() == command {
                if inbound.return_code() != 0 {
                    return Err(SessionError::Protocol);
                }
                return Ok(inbound.payload().to_vec());
            }
            if inbound.sequence() == sequence || !admits_push(inbound.command()) {
                return Err(SessionError::Protocol);
            }
            self.enqueue(inbound)?;
        }
        Err(SessionError::PushLimit)
    }

    /// Reads one authenticated Push while the request path is idle.
    ///
    /// # Errors
    ///
    /// Returns an error if the frame fails authentication or its route is not admitted.
    pub async fn read_push<F>(
        &mut self,
        auth: &SessionAuth<'_>,
        admits_push: F,
    ) -> Result<SsoResponse, SessionError>
    where
        F: Fn(&str) -> bool,
    {
        let inbound = self.read(auth).await?;
        if inbound.return_code() != 0 || !admits_push(inbound.command()) {
            return Err(SessionError::Protocol);
        }
        Ok(inbound)
    }

    async fn read(&mut self, auth: &SessionAuth<'_>) -> Result<SsoResponse, SessionError> {
        let encoded = self.transport.read_frame().await?;
        decode_session_frame(&encoded, auth).map_err(Into::into)
    }

    fn enqueue(&mut self, push: SsoResponse) -> Result<(), SessionError> {
        if self.pushes.len() >= MAX_QUEUED_PUSHES {
            return Err(SessionError::PushLimit);
        }
        self.pushes.push_back(push);
        Ok(())
    }
}
