//! QQ login state boundary for Lirvena.

mod artifact;
mod credential;
mod error;
mod key_agreement;
mod polling;
mod qr_packet;

pub use artifact::QrArtifact;
pub use credential::{
    CredentialExchangeContext, CredentialExchangeError, CredentialExchangeOutcome,
    CredentialExchangeRequest, CredentialLogin, CredentialRejection, CredentialResponseContext,
    CredentialSessionSecrets, build_credential_exchange, decode_credential_exchange_response,
};
pub use error::{QrArtifactError, QrFlowError, UnknownQrPollState};
#[cfg(target_os = "linux")]
pub use key_agreement::LinuxKeyAgreement;
pub use key_agreement::{KeyAgreementError, QqKeyAgreement};
pub use polling::{
    QrControlEvent, QrPollState, accept_qr_artifact, apply_qr_poll, begin_qr_request,
};
pub use qr_packet::{
    QrChallenge, QrDevice, QrFetchContext, QrFetchResponse, QrLoginSecrets, QrPacketError,
    QrPollContext, QrPollResponse, QrResponseContext, QrUnsignedRequest, build_qr_fetch,
    build_qr_poll, decode_qr_fetch_response, decode_qr_poll_response,
};
