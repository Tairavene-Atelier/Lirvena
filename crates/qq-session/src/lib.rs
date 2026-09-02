//! Single-reader authenticated QQ session boundary for Lirvena.

mod error;
mod runtime;

pub use error::SessionError;
pub use runtime::AuthenticatedSession;
