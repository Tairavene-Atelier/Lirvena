use crate::{AccountIdentity, EventHubError};

/// One authenticated group essence-message change with a retained local correlation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedGroupEssence {
    account: AccountIdentity,
    group_id: u64,
    message_id: u32,
    sender_id: u64,
    operator_id: u64,
    added: bool,
    occurred_at: u64,
}

impl ResolvedGroupEssence {
    /// Creates one adapter-neutral essence change.
    ///
    /// # Errors
    ///
    /// Returns an error for any missing identity or message correlation.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        account: AccountIdentity,
        group_id: u64,
        message_id: u32,
        sender_id: u64,
        operator_id: u64,
        added: bool,
        occurred_at: u64,
    ) -> Result<Self, EventHubError> {
        if group_id == 0 || message_id == 0 || sender_id == 0 || operator_id == 0 {
            return Err(EventHubError::InvalidEvent);
        }
        Ok(Self {
            account,
            group_id,
            message_id,
            sender_id,
            operator_id,
            added,
            occurred_at,
        })
    }
    /// Returns the receiving account.
    #[must_use]
    pub const fn account(&self) -> &AccountIdentity {
        &self.account
    }
    /// Returns the QQ group identifier.
    #[must_use]
    pub const fn group_id(&self) -> u64 {
        self.group_id
    }
    /// Returns the retained `OneBot` message identifier.
    #[must_use]
    pub const fn message_id(&self) -> u32 {
        self.message_id
    }
    /// Returns the original message sender.
    #[must_use]
    pub const fn sender_id(&self) -> u64 {
        self.sender_id
    }
    /// Returns the operator who changed the marker.
    #[must_use]
    pub const fn operator_id(&self) -> u64 {
        self.operator_id
    }
    /// Returns whether the marker was added rather than removed.
    #[must_use]
    pub const fn is_added(&self) -> bool {
        self.added
    }
    /// Returns the QQ-supplied Unix event time.
    #[must_use]
    pub const fn occurred_at(&self) -> u64 {
        self.occurred_at
    }
}
