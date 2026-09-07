use crate::{AccountIdentity, EventHubError};

/// One authenticated group-name change ready for protocol adapters.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedGroupNameChange {
    account: AccountIdentity,
    group_id: u64,
    name: String,
    occurred_at: u64,
}

impl ResolvedGroupNameChange {
    /// Creates an adapter-neutral group-name change.
    ///
    /// # Errors
    ///
    /// Returns an error for a missing group or unsafe name.
    pub fn new(
        account: AccountIdentity,
        group_id: u64,
        name: String,
        occurred_at: u64,
    ) -> Result<Self, EventHubError> {
        if group_id == 0 || name.is_empty() || name.len() > 512 || name.contains('\0') {
            return Err(EventHubError::InvalidEvent);
        }
        Ok(Self {
            account,
            group_id,
            name,
            occurred_at,
        })
    }

    /// Returns the receiving account.
    #[must_use]
    pub const fn account(&self) -> &AccountIdentity {
        &self.account
    }

    /// Returns the numeric QQ group identifier.
    #[must_use]
    pub const fn group_id(&self) -> u64 {
        self.group_id
    }

    /// Returns the new group name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the QQ-supplied Unix event time.
    #[must_use]
    pub const fn occurred_at(&self) -> u64 {
        self.occurred_at
    }
}
