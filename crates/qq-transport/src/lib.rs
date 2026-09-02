//! QQ transport boundary for Lirvena.

mod config;
mod error;
mod tcp;

pub use config::{QqEndpoint, TransportConfig};
pub use error::TransportError;
pub use tcp::QqTransport;
