use core::fmt;

use ceylith_protocol::SECURE_FRAME_HEADER_LEN;

use crate::{
    SecureSessionError,
    provider::{MAX_NOISE_MESSAGE_LEN, NOISE_TAG_LEN, TransportCipher},
};

/// Ciphertext overhead added by the fixed transport suite.
pub const TRANSPORT_TAG_LEN: usize = NOISE_TAG_LEN;
/// Default per-direction message cap before session renewal.
pub const DEFAULT_SESSION_MESSAGE_LIMIT: u64 = 1 << 20;

/// Stateful bidirectional secure transport with strict external counters.
pub struct SecureSession {
    send: TransportCipher,
    receive: TransportCipher,
    message_limit: u64,
    closed: bool,
}

impl SecureSession {
    pub(crate) const fn new(send: TransportCipher, receive: TransportCipher) -> Self {
        Self {
            send,
            receive,
            message_limit: DEFAULT_SESSION_MESSAGE_LIMIT,
            closed: false,
        }
    }

    /// Overrides the compiled cap with a smaller release or test limit.
    ///
    /// # Errors
    ///
    /// Returns an error for zero or a value above the compiled cap.
    pub fn with_message_limit(mut self, message_limit: u64) -> Result<Self, SecureSessionError> {
        if message_limit == 0 || message_limit > DEFAULT_SESSION_MESSAGE_LIMIT {
            return Err(SecureSessionError::InvalidLimit);
        }
        self.message_limit = message_limit;
        Ok(self)
    }

    /// Exact next outbound counter.
    #[must_use]
    pub fn next_send_counter(&self) -> u64 {
        self.send.get_next_n()
    }

    /// Exact next inbound counter.
    #[must_use]
    pub fn next_receive_counter(&self) -> u64 {
        self.receive.get_next_n()
    }

    /// Encrypts one payload and authenticates the complete fixed frame header.
    ///
    /// # Errors
    ///
    /// Returns an error for closed, expired, out-of-order, or oversized state/input.
    pub fn seal(
        &mut self,
        counter: u64,
        associated_header: &[u8; SECURE_FRAME_HEADER_LEN],
        plaintext: &[u8],
    ) -> Result<Vec<u8>, SecureSessionError> {
        self.check_open()?;
        if counter != self.send.get_next_n() {
            self.closed = true;
            return Err(SecureSessionError::CounterMismatch);
        }
        if counter >= self.message_limit {
            self.closed = true;
            return Err(SecureSessionError::SessionExpired);
        }
        if plaintext.len() > MAX_NOISE_MESSAGE_LEN - NOISE_TAG_LEN {
            return Err(SecureSessionError::MessageTooLarge);
        }

        let mut ciphertext = vec![0_u8; plaintext.len() + NOISE_TAG_LEN];
        self.send
            .encrypt_ad(associated_header, plaintext, &mut ciphertext);
        Ok(ciphertext)
    }

    /// Decrypts one in-order payload and closes on authentication or counter failure.
    ///
    /// # Errors
    ///
    /// Returns an error for closed, expired, out-of-order, oversized, or unauthenticated input.
    pub fn open(
        &mut self,
        counter: u64,
        associated_header: &[u8; SECURE_FRAME_HEADER_LEN],
        ciphertext: &[u8],
    ) -> Result<Vec<u8>, SecureSessionError> {
        self.check_open()?;
        if counter != self.receive.get_next_n() {
            self.closed = true;
            return Err(SecureSessionError::CounterMismatch);
        }
        if counter >= self.message_limit {
            self.closed = true;
            return Err(SecureSessionError::SessionExpired);
        }
        if !(NOISE_TAG_LEN..=MAX_NOISE_MESSAGE_LEN).contains(&ciphertext.len()) {
            return Err(SecureSessionError::MessageTooLarge);
        }

        let mut plaintext = vec![0_u8; ciphertext.len() - NOISE_TAG_LEN];
        if self
            .receive
            .decrypt_ad(associated_header, ciphertext, &mut plaintext)
            .is_err()
        {
            self.closed = true;
            return Err(SecureSessionError::AuthenticationFailed);
        }
        Ok(plaintext)
    }

    /// Whether the state has terminally closed.
    #[must_use]
    pub const fn is_closed(&self) -> bool {
        self.closed
    }

    fn check_open(&self) -> Result<(), SecureSessionError> {
        if self.closed {
            Err(SecureSessionError::SessionClosed)
        } else {
            Ok(())
        }
    }
}

impl fmt::Debug for SecureSession {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SecureSession")
            .field("next_send_counter", &self.next_send_counter())
            .field("next_receive_counter", &self.next_receive_counter())
            .field("message_limit", &self.message_limit)
            .field("closed", &self.closed)
            .finish_non_exhaustive()
    }
}
