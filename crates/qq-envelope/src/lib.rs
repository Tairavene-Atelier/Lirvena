//! QQ envelope construction boundary for Lirvena.

mod marked;
mod tea;
mod transport;

pub use marked::{EnvelopeMark, encode_marked_reserve};
pub use tea::{QqTeaError, QqTeaKey, decrypt_qq_tea, encrypt_qq_tea, encrypt_qq_tea_with_padding};
pub use transport::{
    EnvelopeError, ExpectedSsoResponse, ServiceFrameParts, ServiceResponse, SessionAuth,
    SessionRequestParts, SsoRequestParts, SsoResponse, decode_service_response,
    decode_session_frame, decode_session_response, decode_sso_response, encode_service_frame,
    encode_session_request, encode_sso_request,
};
