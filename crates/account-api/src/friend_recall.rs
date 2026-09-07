use crate::AccountIdentity;

/// One direct-message recall with a resolved author and retained local correlation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedFriendRecall {
    account: AccountIdentity,
    user_id: u64,
    message_id: u32,
    tip: String,
    occurred_at: u64,
}

impl ResolvedFriendRecall {
    /// Creates an adapter-neutral friend recall notice.
    ///
    /// # Errors
    ///
    /// Returns an error for zero identifiers or unsafe tip text.
    pub fn new(
        account: AccountIdentity,
        user_id: u64,
        message_id: u32,
        tip: String,
        occurred_at: u64,
    ) -> Result<Self, crate::EventHubError> {
        if user_id == 0 || message_id == 0 || tip.len() > 2048 || tip.chars().any(char::is_control)
        {
            return Err(crate::EventHubError::InvalidEvent);
        }
        Ok(Self {
            account,
            user_id,
            message_id,
            tip,
            occurred_at,
        })
    }
    /// Returns the receiving account identity.
    #[must_use]
    pub const fn account(&self) -> &AccountIdentity {
        &self.account
    }
    /// Returns the recalling author's numeric QQ identifier.
    #[must_use]
    pub const fn user_id(&self) -> u64 {
        self.user_id
    }
    /// Returns the retained local `OneBot` message identifier.
    #[must_use]
    pub const fn message_id(&self) -> u32 {
        self.message_id
    }
    /// Returns QQ's human-readable recall tip.
    #[must_use]
    pub fn tip(&self) -> &str {
        &self.tip
    }
    /// Returns the QQ-supplied Unix event time.
    #[must_use]
    pub const fn occurred_at(&self) -> u64 {
        self.occurred_at
    }
}
