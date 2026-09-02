use ceylith_crypto::{ClientHandshake, NoisePublicKey};
use ceylith_protocol::{
    HandshakeEnvelope, HandshakeStep, WireLimits, decode_handshake_envelope,
    decode_session_welcome, encode_handshake_envelope, proto,
};
use prost::Message;
use zeroize::{Zeroize, Zeroizing};

use crate::{AccessToken, ClientConnection, ClientError, InstallationIdentity, RuntimeDescriptor};

/// Client handshake state awaiting one authenticated server response.
pub struct PendingHandshake {
    secure_handshake: ClientHandshake,
    limits: WireLimits,
}

impl PendingHandshake {
    /// Creates and encodes the single client handshake message.
    ///
    /// # Errors
    ///
    /// Returns an error when identity encoding, entropy, bounds, or encryption fails.
    pub fn start(
        identity: &InstallationIdentity,
        server_static_key: NoisePublicKey,
        token: Option<&AccessToken>,
        runtime: &RuntimeDescriptor,
        requested_feature_bits: u64,
        limits: WireLimits,
    ) -> Result<(Self, Vec<u8>), ClientError> {
        let mut hello = identity.signed_hello(token, runtime, requested_feature_bits)?;
        let mut payload = Zeroizing::new(Vec::with_capacity(hello.encoded_len()));
        hello.encode(&mut *payload)?;
        hello.access_token.zeroize();

        let (secure_handshake, message) =
            ClientHandshake::start(identity.noise_private_key(), server_static_key, &payload)?;
        let envelope = HandshakeEnvelope::new(HandshakeStep::ClientHello, message, limits)?;
        let encoded = encode_handshake_envelope(&envelope, limits)?;
        Ok((
            Self {
                secure_handshake,
                limits,
            },
            encoded,
        ))
    }

    /// Authenticates one server response and enters secure transport mode.
    ///
    /// # Errors
    ///
    /// Returns an error when the response is malformed, unauthenticated, or inadmissible.
    pub fn finish(self, encoded: &[u8]) -> Result<ClientConnection, ClientError> {
        let envelope = decode_handshake_envelope(encoded, self.limits)?;
        if envelope.step() != HandshakeStep::ServerWelcome {
            return Err(ClientError::Protocol);
        }
        let (payload, secure_session, _) = self.secure_handshake.finish(envelope.payload())?;
        let payload = Zeroizing::new(payload);
        let welcome = proto::SessionWelcome::decode(payload.as_slice())?;
        let admission = decode_session_welcome(&welcome)?;
        Ok(ClientConnection::new(
            admission,
            secure_session,
            self.limits,
        ))
    }
}

impl core::fmt::Debug for PendingHandshake {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("PendingHandshake(<active>)")
    }
}
