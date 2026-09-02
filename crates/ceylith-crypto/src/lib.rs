#![doc = "Fixed-suite secure session shared by Ceylith and Lirvena."]

mod error;
mod handshake;
mod keys;
mod provider;
mod session;

pub use error::SecureSessionError;
pub use handshake::{ClientHandshake, HandshakeBinding, ServerHandshake, ServerHandshakeResponse};
pub use keys::{NoisePrivateKey, NoisePublicKey};
pub use session::{DEFAULT_SESSION_MESSAGE_LIMIT, SecureSession, TRANSPORT_TAG_LEN};
