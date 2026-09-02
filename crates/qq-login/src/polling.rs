use qq_domain::{LoginMachine, LoginState};

use crate::{QrArtifact, QrFlowError, UnknownQrPollState};

/// Closed QR polling states used by the compiled QQ login implementation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QrPollState {
    /// Temporary credentials are ready for the next login exchange.
    Confirmed,
    /// The QR code expired.
    Expired,
    /// The QR code has not been scanned.
    WaitingForScan,
    /// The QR code was scanned and awaits user confirmation.
    WaitingForConfirmation,
    /// The user canceled this QR login.
    Canceled,
}

impl TryFrom<u8> for QrPollState {
    type Error = UnknownQrPollState;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Confirmed),
            17 => Ok(Self::Expired),
            48 => Ok(Self::WaitingForScan),
            53 => Ok(Self::WaitingForConfirmation),
            54 => Ok(Self::Canceled),
            other => Err(UnknownQrPollState::new(other)),
        }
    }
}

/// Redacted login control event emitted for presentation and adapters.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QrControlEvent {
    /// A validated QR artifact became available.
    Available {
        /// Local event time.
        occurred_at_ms: u64,
        /// Exclusive QR expiry.
        expires_at_ms: u64,
        /// SHA-256 digest of the PNG artifact.
        png_sha256: [u8; 32],
    },
    /// The QR flow changed state.
    StateChanged {
        /// Local event time.
        occurred_at_ms: u64,
        /// Prior login state.
        previous: LoginState,
        /// Current login state.
        current: LoginState,
    },
}

/// Starts a fresh QR request.
///
/// # Errors
///
/// Returns an error when the account state does not allow a fresh request.
pub fn begin_qr_request(
    machine: &mut LoginMachine,
    occurred_at_ms: u64,
) -> Result<QrControlEvent, QrFlowError> {
    transition_event(machine, LoginState::FetchingQr, occurred_at_ms)
}

/// Accepts a validated artifact and moves the flow to scan polling.
///
/// # Errors
///
/// Returns an error when no QR request is active.
pub fn accept_qr_artifact(
    machine: &mut LoginMachine,
    artifact: &QrArtifact,
    occurred_at_ms: u64,
) -> Result<QrControlEvent, QrFlowError> {
    machine.transition(LoginState::AwaitingScan)?;
    Ok(QrControlEvent::Available {
        occurred_at_ms,
        expires_at_ms: artifact.expires_at_ms(),
        png_sha256: artifact.png_sha256(),
    })
}

/// Applies one known remote polling state.
///
/// # Errors
///
/// Returns an error when the transition contradicts the local flow state.
pub fn apply_qr_poll(
    machine: &mut LoginMachine,
    state: QrPollState,
    occurred_at_ms: u64,
) -> Result<QrControlEvent, QrFlowError> {
    let next = match state {
        QrPollState::Confirmed => LoginState::ExchangingCredentials,
        QrPollState::Expired => LoginState::QrExpired,
        QrPollState::WaitingForScan => LoginState::AwaitingScan,
        QrPollState::WaitingForConfirmation => LoginState::AwaitingConfirmation,
        QrPollState::Canceled => LoginState::QrCanceled,
    };
    transition_event(machine, next, occurred_at_ms)
}

fn transition_event(
    machine: &mut LoginMachine,
    next: LoginState,
    occurred_at_ms: u64,
) -> Result<QrControlEvent, QrFlowError> {
    let previous = machine.transition(next)?;
    Ok(QrControlEvent::StateChanged {
        occurred_at_ms,
        previous,
        current: next,
    })
}
