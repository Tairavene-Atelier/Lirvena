use crate::AccountIdentity;

/// One recalled group message with resolved identities and local message correlation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedGroupRecall {
    account: AccountIdentity,
    group_id: u64,
    user_id: u64,
    operator_id: u64,
    message_id: u32,
    tip: String,
    occurred_at: u64,
}

impl ResolvedGroupRecall {
    /// Creates an adapter-neutral recall notice.
    ///
    /// # Errors
    ///
    /// Returns an error for zero identities, a zero local message ID, or unsafe tip text.
    pub fn new(
        account: AccountIdentity,
        group_id: u64,
        user_id: u64,
        operator_id: u64,
        message_id: u32,
        tip: String,
        occurred_at: u64,
    ) -> Result<Self, crate::EventHubError> {
        if group_id == 0
            || user_id == 0
            || operator_id == 0
            || message_id == 0
            || tip.len() > 2048
            || tip.chars().any(char::is_control)
        {
            return Err(crate::EventHubError::InvalidEvent);
        }
        Ok(Self {
            account,
            group_id,
            user_id,
            operator_id,
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
    /// Returns the group identifier.
    #[must_use]
    pub const fn group_id(&self) -> u64 {
        self.group_id
    }
    /// Returns the original author identifier.
    #[must_use]
    pub const fn user_id(&self) -> u64 {
        self.user_id
    }
    /// Returns the recall operator identifier.
    #[must_use]
    pub const fn operator_id(&self) -> u64 {
        self.operator_id
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
