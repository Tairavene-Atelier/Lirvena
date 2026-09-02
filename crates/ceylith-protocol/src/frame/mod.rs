mod handshake;
mod secure;

use crate::{
    CodecError, HARD_MAX_OUTER_FRAME_LEN, LengthKind, RequestId, SessionId,
    bounds::{DEFAULT_INNER_FRAME_LEN, DEFAULT_SECURE_CIPHERTEXT_LEN},
};

pub use handshake::{decode_handshake_envelope, encode_handshake_envelope};
pub use secure::{decode_secure_frame, encode_secure_frame, encode_secure_frame_header};

/// Current public outer wire version.
pub const CURRENT_WIRE_VERSION: u16 = 2;
/// Fixed magic for a Ceylith v2 handshake envelope.
pub const HANDSHAKE_MAGIC: [u8; 4] = *b"CYH2";
/// Fixed byte length before a handshake payload.
pub const HANDSHAKE_HEADER_LEN: usize = 12;
/// Fixed magic for a Ceylith v2 encrypted frame.
pub const SECURE_FRAME_MAGIC: [u8; 4] = *b"CYF2";
/// Fixed byte length before encrypted frame ciphertext.
pub const SECURE_FRAME_HEADER_LEN: usize = 52;

const HARD_MAX_HANDSHAKE_PAYLOAD_LEN: usize = HARD_MAX_OUTER_FRAME_LEN - HANDSHAKE_HEADER_LEN;
const HARD_MAX_CIPHERTEXT_LEN: usize = HARD_MAX_OUTER_FRAME_LEN - SECURE_FRAME_HEADER_LEN;

/// Handshake direction encoded in the fixed envelope header.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum HandshakeStep {
    /// Initiator message.
    ClientHello = 1,
    /// Responder message.
    ServerWelcome = 2,
}

impl TryFrom<u8> for HandshakeStep {
    type Error = CodecError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::ClientHello),
            2 => Ok(Self::ServerWelcome),
            _ => Err(CodecError::InvalidHandshakeStep),
        }
    }
}

/// Runtime-configurable limits that may only tighten compiled hard bounds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WireLimits {
    handshake_payload: usize,
    ciphertext: usize,
    inner_frame: usize,
}

impl WireLimits {
    /// Creates limits no larger than the compiled hard bounds.
    ///
    /// # Errors
    ///
    /// Returns [`CodecError::LengthLimitExceeded`] when any supplied limit is
    /// larger than its compiled hard bound.
    pub const fn new(
        max_handshake_payload_len: usize,
        max_ciphertext_len: usize,
        max_inner_frame_len: usize,
    ) -> Result<Self, CodecError> {
        if max_handshake_payload_len > HARD_MAX_HANDSHAKE_PAYLOAD_LEN {
            return Err(CodecError::LengthLimitExceeded {
                kind: LengthKind::HandshakePayload,
                limit: HARD_MAX_HANDSHAKE_PAYLOAD_LEN,
                actual: max_handshake_payload_len,
            });
        }
        if max_ciphertext_len > HARD_MAX_CIPHERTEXT_LEN {
            return Err(CodecError::LengthLimitExceeded {
                kind: LengthKind::Ciphertext,
                limit: HARD_MAX_CIPHERTEXT_LEN,
                actual: max_ciphertext_len,
            });
        }
        if max_inner_frame_len > HARD_MAX_OUTER_FRAME_LEN {
            return Err(CodecError::LengthLimitExceeded {
                kind: LengthKind::InnerFrame,
                limit: HARD_MAX_OUTER_FRAME_LEN,
                actual: max_inner_frame_len,
            });
        }
        Ok(Self {
            handshake_payload: max_handshake_payload_len,
            ciphertext: max_ciphertext_len,
            inner_frame: max_inner_frame_len,
        })
    }

    /// Maximum handshake payload length.
    #[must_use]
    pub const fn max_handshake_payload_len(self) -> usize {
        self.handshake_payload
    }

    /// Maximum encrypted payload length.
    #[must_use]
    pub const fn max_ciphertext_len(self) -> usize {
        self.ciphertext
    }

    /// Maximum decoded inner frame length.
    #[must_use]
    pub const fn max_inner_frame_len(self) -> usize {
        self.inner_frame
    }
}

impl Default for WireLimits {
    fn default() -> Self {
        Self {
            handshake_payload: HARD_MAX_HANDSHAKE_PAYLOAD_LEN,
            ciphertext: DEFAULT_SECURE_CIPHERTEXT_LEN,
            inner_frame: DEFAULT_INNER_FRAME_LEN,
        }
    }
}

/// One bounded fixed-header handshake envelope.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HandshakeEnvelope {
    step: HandshakeStep,
    payload: Box<[u8]>,
}

impl HandshakeEnvelope {
    /// Validates an envelope under the supplied limits.
    ///
    /// # Errors
    ///
    /// Returns [`CodecError::LengthLimitExceeded`] when the payload is too large.
    pub fn new(
        step: HandshakeStep,
        payload: Vec<u8>,
        limits: WireLimits,
    ) -> Result<Self, CodecError> {
        enforce_limit(
            LengthKind::HandshakePayload,
            payload.len(),
            limits.handshake_payload,
        )?;
        Ok(Self {
            step,
            payload: payload.into_boxed_slice(),
        })
    }

    /// Handshake direction.
    #[must_use]
    pub const fn step(&self) -> HandshakeStep {
        self.step
    }

    /// Opaque Noise handshake payload.
    #[must_use]
    pub fn payload(&self) -> &[u8] {
        &self.payload
    }
}

/// One bounded encrypted frame.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SecureFrame {
    session_id: SessionId,
    counter: u64,
    request_id: RequestId,
    ciphertext: Box<[u8]>,
}

impl SecureFrame {
    /// Validates an encrypted frame under the supplied limits.
    ///
    /// # Errors
    ///
    /// Returns [`CodecError::LengthLimitExceeded`] when the ciphertext is too large.
    pub fn new(
        session_id: SessionId,
        counter: u64,
        request_id: RequestId,
        ciphertext: Vec<u8>,
        limits: WireLimits,
    ) -> Result<Self, CodecError> {
        enforce_limit(LengthKind::Ciphertext, ciphertext.len(), limits.ciphertext)?;
        Ok(Self {
            session_id,
            counter,
            request_id,
            ciphertext: ciphertext.into_boxed_slice(),
        })
    }

    /// Secure-session identifier.
    #[must_use]
    pub const fn session_id(&self) -> SessionId {
        self.session_id
    }

    /// Direction-specific monotonic counter.
    #[must_use]
    pub const fn counter(&self) -> u64 {
        self.counter
    }

    /// Logical request identifier.
    #[must_use]
    pub const fn request_id(&self) -> RequestId {
        self.request_id
    }

    /// Authenticated ciphertext.
    #[must_use]
    pub fn ciphertext(&self) -> &[u8] {
        &self.ciphertext
    }
}

pub(super) fn validate_version(version: u16) -> Result<(), CodecError> {
    if version == CURRENT_WIRE_VERSION {
        Ok(())
    } else {
        Err(CodecError::UnsupportedVersion)
    }
}

pub(super) fn decode_u32_len(bytes: &[u8]) -> Result<usize, CodecError> {
    let encoded: [u8; 4] = bytes.try_into().map_err(|_| CodecError::LengthOverflow)?;
    usize::try_from(u32::from_be_bytes(encoded)).map_err(|_| CodecError::LengthOverflow)
}

pub(super) fn enforce_limit(
    kind: LengthKind,
    actual: usize,
    limit: usize,
) -> Result<(), CodecError> {
    if actual > limit {
        Err(CodecError::LengthLimitExceeded {
            kind,
            limit,
            actual,
        })
    } else {
        Ok(())
    }
}

pub(super) fn require_header(input: &[u8], needed: usize) -> Result<(), CodecError> {
    if input.len() < needed {
        Err(CodecError::Truncated {
            needed,
            available: input.len(),
        })
    } else {
        Ok(())
    }
}

pub(super) fn require_exact_len(input: &[u8], expected: usize) -> Result<(), CodecError> {
    if input.len() < expected {
        return Err(CodecError::Truncated {
            needed: expected,
            available: input.len(),
        });
    }
    if input.len() > expected {
        return Err(CodecError::TrailingBytes {
            expected,
            actual: input.len(),
        });
    }
    Ok(())
}
