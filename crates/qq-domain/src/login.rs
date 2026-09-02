use core::fmt;

/// Closed account login state exposed to runtime adapters.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LoginState {
    /// The account transport is stopped.
    Stopped,
    /// A new QR code is being requested.
    FetchingQr,
    /// The QR code is waiting to be scanned.
    AwaitingScan,
    /// The QR code was scanned and awaits confirmation.
    AwaitingConfirmation,
    /// Temporary credentials are being exchanged.
    ExchangingCredentials,
    /// Complete credentials are awaiting QQ online registration.
    Registering,
    /// The account is online.
    Online,
    /// The account was closed to preserve a required security property.
    ProtectiveOffline,
    /// Login stopped after a classified failure.
    Failed(LoginFailure),
    /// The QR code expired before confirmation.
    QrExpired,
    /// The QR code was canceled by its user.
    QrCanceled,
}

/// Redacted login failure category.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LoginFailure {
    /// The remote transport failed.
    Transport,
    /// A remote packet violated the compiled protocol contract.
    Protocol,
    /// No compatible profile was available.
    Profile,
    /// Required Ceylith continuity was lost.
    CeylithContinuity,
    /// QQ rejected the login attempt.
    Rejected,
}

/// State transition failure without account or credential data.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TransitionError {
    from: LoginState,
    to: LoginState,
}

impl TransitionError {
    /// Returns the source state.
    #[must_use]
    pub const fn from(self) -> LoginState {
        self.from
    }

    /// Returns the rejected destination state.
    #[must_use]
    pub const fn to(self) -> LoginState {
        self.to
    }
}

impl fmt::Display for TransitionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("login state transition rejected")
    }
}

impl std::error::Error for TransitionError {}

/// Single-owner login state machine.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LoginMachine {
    state: LoginState,
}

impl Default for LoginMachine {
    fn default() -> Self {
        Self {
            state: LoginState::Stopped,
        }
    }
}

impl LoginMachine {
    /// Creates a stopped login machine.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            state: LoginState::Stopped,
        }
    }

    /// Returns the current state.
    #[must_use]
    pub const fn state(self) -> LoginState {
        self.state
    }

    /// Applies one checked transition and returns the prior state.
    ///
    /// # Errors
    ///
    /// Returns an error when `next` is not reachable from the current state.
    pub fn transition(&mut self, next: LoginState) -> Result<LoginState, TransitionError> {
        let previous = self.state;
        if !is_allowed(previous, next) {
            return Err(TransitionError {
                from: previous,
                to: next,
            });
        }
        self.state = next;
        Ok(previous)
    }
}

const fn is_allowed(from: LoginState, to: LoginState) -> bool {
    use LoginState::{
        AwaitingConfirmation, AwaitingScan, ExchangingCredentials, Failed, FetchingQr, Online,
        ProtectiveOffline, QrCanceled, QrExpired, Registering, Stopped,
    };

    matches!(
        (from, to),
        (
            Stopped | ProtectiveOffline | Failed(_) | QrExpired | QrCanceled,
            FetchingQr
        ) | (FetchingQr, AwaitingScan | Failed(_) | Stopped)
            | (
                AwaitingScan | AwaitingConfirmation,
                AwaitingScan
                    | AwaitingConfirmation
                    | ExchangingCredentials
                    | QrExpired
                    | QrCanceled
                    | Failed(_)
                    | Stopped
            )
            | (
                ExchangingCredentials,
                Registering | ProtectiveOffline | Failed(_) | Stopped
            )
            | (
                Registering,
                Online | ProtectiveOffline | Failed(_) | Stopped
            )
            | (Online, ProtectiveOffline | Failed(_) | Stopped)
            | (_, Stopped)
    )
}
