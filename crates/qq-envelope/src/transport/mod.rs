mod error;
mod service;
mod session;
mod sso;

pub use error::EnvelopeError;
pub use service::{
    ServiceFrameParts, ServiceResponse, decode_service_response, encode_service_frame,
};
pub use session::{
    ExpectedSsoResponse, SessionAuth, SessionRequestParts, decode_session_frame,
    decode_session_response, encode_session_request,
};
pub use sso::{SsoRequestParts, SsoResponse, decode_sso_response, encode_sso_request};

pub(super) const MAX_PACKET_LEN: usize = 2 * 1024 * 1024;
