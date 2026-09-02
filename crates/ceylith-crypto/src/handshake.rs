use core::fmt;

use crate::{
    NoisePrivateKey, NoisePublicKey, SecureSession, SecureSessionError,
    provider::{IkState, MAX_NOISE_MESSAGE_LEN, initiator_state, responder_state},
};

/// Opaque 32-byte handshake hash for channel binding.
#[derive(Clone, Copy, Eq, Hash, PartialEq)]
pub struct HandshakeBinding([u8; 32]);

impl HandshakeBinding {
    fn from_slice(bytes: &[u8]) -> Result<Self, SecureSessionError> {
        let binding = bytes
            .try_into()
            .map_err(|_| SecureSessionError::HandshakeFailed)?;
        Ok(Self(binding))
    }

    /// Borrows the exact channel-binding bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Debug for HandshakeBinding {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("HandshakeBinding(<opaque>)")
    }
}

/// Client-side fixed-suite handshake before the response is consumed.
pub struct ClientHandshake {
    state: IkState,
}

impl ClientHandshake {
    /// Builds the authenticated initiator message for a trusted server key.
    ///
    /// # Errors
    ///
    /// Returns an error when entropy, bounds, or the fixed handshake fails.
    pub fn start(
        static_key: &NoisePrivateKey,
        server_static_key: NoisePublicKey,
        payload: &[u8],
    ) -> Result<(Self, Vec<u8>), SecureSessionError> {
        let ephemeral = NoisePrivateKey::generate()?;
        let mut state = initiator_state(
            static_key.sensitive(),
            ephemeral.sensitive(),
            *server_static_key.as_bytes(),
        );
        enforce_handshake_payload_len(&state, payload.len())?;
        let message = state
            .write_message_vec(payload)
            .map_err(|_| SecureSessionError::HandshakeFailed)?;
        Ok((Self { state }, message))
    }

    /// Authenticates the response and enters transport mode.
    ///
    /// # Errors
    ///
    /// Returns an error when the response is oversized, unauthenticated, or incomplete.
    pub fn finish(
        mut self,
        response: &[u8],
    ) -> Result<(Vec<u8>, SecureSession, HandshakeBinding), SecureSessionError> {
        enforce_message_len(response.len())?;
        let payload = self
            .state
            .read_message_vec(response)
            .map_err(|_| SecureSessionError::HandshakeFailed)?;
        if !self.state.completed() {
            return Err(SecureSessionError::HandshakeFailed);
        }
        let binding = HandshakeBinding::from_slice(self.state.get_hash())?;
        let (send, receive) = self.state.get_ciphers();
        Ok((payload, SecureSession::new(send, receive), binding))
    }
}

impl fmt::Debug for ClientHandshake {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ClientHandshake(<active>)")
    }
}

/// Server-side fixed-suite handshake factory.
pub struct ServerHandshake {
    static_key: NoisePrivateKey,
}

impl ServerHandshake {
    /// Creates a server handshake factory using one provisioned static key.
    #[must_use]
    pub const fn new(static_key: NoisePrivateKey) -> Self {
        Self { static_key }
    }

    /// Public key distributed through a separately authenticated trust document.
    #[must_use]
    pub fn public_key(&self) -> NoisePublicKey {
        self.static_key.public_key()
    }

    /// Authenticates and decrypts the initiator message.
    ///
    /// # Errors
    ///
    /// Returns an error when entropy, bounds, peer authentication, or decryption fails.
    pub fn begin(
        &self,
        message: &[u8],
    ) -> Result<(ServerHandshakeResponse, Vec<u8>), SecureSessionError> {
        enforce_message_len(message.len())?;
        let ephemeral = NoisePrivateKey::generate()?;
        let mut state = responder_state(self.static_key.sensitive(), ephemeral.sensitive());
        let payload = state
            .read_message_vec(message)
            .map_err(|_| SecureSessionError::HandshakeFailed)?;
        let remote = state
            .get_rs()
            .and_then(|bytes| NoisePublicKey::try_from_bytes(bytes).ok())
            .ok_or(SecureSessionError::HandshakeFailed)?;
        Ok((ServerHandshakeResponse { state, remote }, payload))
    }
}

impl fmt::Debug for ServerHandshake {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ServerHandshake(<provisioned>)")
    }
}

/// Authenticated server handshake awaiting its response payload.
pub struct ServerHandshakeResponse {
    state: IkState,
    remote: NoisePublicKey,
}

impl ServerHandshakeResponse {
    /// Authenticated initiator static public key.
    #[must_use]
    pub const fn remote_static_key(&self) -> NoisePublicKey {
        self.remote
    }

    /// Encrypts the response and enters transport mode.
    ///
    /// # Errors
    ///
    /// Returns an error when the payload is oversized or the handshake cannot complete.
    pub fn finish(
        mut self,
        payload: &[u8],
    ) -> Result<(Vec<u8>, SecureSession, HandshakeBinding), SecureSessionError> {
        enforce_handshake_payload_len(&self.state, payload.len())?;
        let response = self
            .state
            .write_message_vec(payload)
            .map_err(|_| SecureSessionError::HandshakeFailed)?;
        if !self.state.completed() {
            return Err(SecureSessionError::HandshakeFailed);
        }
        let binding = HandshakeBinding::from_slice(self.state.get_hash())?;
        let (receive, send) = self.state.get_ciphers();
        Ok((response, SecureSession::new(send, receive), binding))
    }
}

impl fmt::Debug for ServerHandshakeResponse {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ServerHandshakeResponse(<active>)")
    }
}

fn enforce_handshake_payload_len(
    state: &IkState,
    payload_len: usize,
) -> Result<(), SecureSessionError> {
    let message_len = payload_len
        .checked_add(state.get_next_message_overhead())
        .ok_or(SecureSessionError::MessageTooLarge)?;
    enforce_message_len(message_len)
}

fn enforce_message_len(message_len: usize) -> Result<(), SecureSessionError> {
    if message_len > MAX_NOISE_MESSAGE_LEN {
        Err(SecureSessionError::MessageTooLarge)
    } else {
        Ok(())
    }
}
