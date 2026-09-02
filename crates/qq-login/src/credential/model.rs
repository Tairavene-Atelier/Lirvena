use zeroize::Zeroize;

/// Validated result of one post-QR credential exchange.
pub enum CredentialExchangeOutcome {
    /// QQ issued the complete session material required by the next login stage.
    Success(CredentialLogin),
    /// QQ rejected the exchange or requested an unsupported interactive step.
    Rejected(CredentialRejection),
}

impl core::fmt::Debug for CredentialExchangeOutcome {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Success(login) => formatter.debug_tuple("Success").field(login).finish(),
            Self::Rejected(rejection) => {
                formatter.debug_tuple("Rejected").field(rejection).finish()
            }
        }
    }
}

/// Ordinary account facts and zeroizing session material returned after QR login.
pub struct CredentialLogin {
    uid: String,
    nickname: String,
    age: u8,
    gender: u8,
    secrets: CredentialSessionSecrets,
}

impl CredentialLogin {
    pub(super) fn new(
        uid: String,
        nickname: String,
        age: u8,
        gender: u8,
        secrets: CredentialSessionSecrets,
    ) -> Self {
        Self {
            uid,
            nickname,
            age,
            gender,
            secrets,
        }
    }

    /// Returns the QQ UID bound to this session.
    #[must_use]
    pub fn uid(&self) -> &str {
        &self.uid
    }

    /// Returns the profile nickname received during login.
    #[must_use]
    pub fn nickname(&self) -> &str {
        &self.nickname
    }

    /// Returns the profile age byte received during login.
    #[must_use]
    pub const fn age(&self) -> u8 {
        self.age
    }

    /// Returns the profile gender byte received during login.
    #[must_use]
    pub const fn gender(&self) -> u8 {
        self.gender
    }

    /// Borrows the zeroizing session material for the registration stage.
    #[must_use]
    pub const fn secrets(&self) -> &CredentialSessionSecrets {
        &self.secrets
    }
}

impl core::fmt::Debug for CredentialLogin {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("CredentialLogin")
            .field("uid", &self.uid)
            .field("nickname", &self.nickname)
            .field("age", &self.age)
            .field("gender", &self.gender)
            .field("secrets", &self.secrets)
            .finish()
    }
}

/// Session values retained for the next QQ transport generation.
pub struct CredentialSessionSecrets {
    d2_key: Box<[u8]>,
    tgt: Box<[u8]>,
    d2: Box<[u8]>,
    temporary_password: Box<[u8]>,
}

impl CredentialSessionSecrets {
    pub(super) fn new(d2_key: &[u8], tgt: &[u8], d2: &[u8], temporary_password: &[u8]) -> Self {
        Self {
            d2_key: d2_key.to_vec().into_boxed_slice(),
            tgt: tgt.to_vec().into_boxed_slice(),
            d2: d2.to_vec().into_boxed_slice(),
            temporary_password: temporary_password.to_vec().into_boxed_slice(),
        }
    }

    /// Borrows the D2 envelope key.
    #[must_use]
    pub fn d2_key(&self) -> &[u8] {
        &self.d2_key
    }

    /// Borrows the TGT credential.
    #[must_use]
    pub fn tgt(&self) -> &[u8] {
        &self.tgt
    }

    /// Borrows the D2 credential.
    #[must_use]
    pub fn d2(&self) -> &[u8] {
        &self.d2
    }

    /// Borrows the temporary login password material.
    #[must_use]
    pub fn temporary_password(&self) -> &[u8] {
        &self.temporary_password
    }
}

impl core::fmt::Debug for CredentialSessionSecrets {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("CredentialSessionSecrets")
            .field("d2_key", &"<redacted>")
            .field("tgt", &"<redacted>")
            .field("d2", &"<redacted>")
            .field("temporary_password", &"<redacted>")
            .finish()
    }
}

impl Drop for CredentialSessionSecrets {
    fn drop(&mut self) {
        self.d2_key.zeroize();
        self.tgt.zeroize();
        self.d2.zeroize();
        self.temporary_password.zeroize();
    }
}

/// Bounded ordinary explanation for a rejected credential exchange.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CredentialRejection {
    state: u8,
    tag: Option<String>,
    message: Option<String>,
}

impl CredentialRejection {
    pub(super) const fn new(state: u8, tag: Option<String>, message: Option<String>) -> Self {
        Self {
            state,
            tag,
            message,
        }
    }

    /// Returns the QQ login state byte.
    #[must_use]
    pub const fn state(&self) -> u8 {
        self.state
    }

    /// Returns QQ's optional bounded category text.
    #[must_use]
    pub fn tag(&self) -> Option<&str> {
        self.tag.as_deref()
    }

    /// Returns QQ's optional bounded user-facing message.
    #[must_use]
    pub fn message(&self) -> Option<&str> {
        self.message.as_deref()
    }
}
