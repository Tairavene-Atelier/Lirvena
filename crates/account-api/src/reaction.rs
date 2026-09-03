use crate::{AccountIdentity, EventHubError};

/// One authenticated group-message reaction with all adapter identifiers resolved locally.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedGroupReaction {
    account: AccountIdentity,
    group_id: u64,
    message_id: u32,
    operator_id: u64,
    add: bool,
    code: String,
    count: u32,
    occurred_at: u64,
}

impl ResolvedGroupReaction {
    /// Creates an adapter-neutral reaction event.
    ///
    /// # Errors
    ///
    /// Returns an error for missing identities, message correlation, or an invalid reaction code.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        account: AccountIdentity,
        group_id: u64,
        message_id: u32,
        operator_id: u64,
        add: bool,
        code: String,
        count: u32,
        occurred_at: u64,
    ) -> Result<Self, EventHubError> {
        if group_id == 0
            || message_id == 0
            || operator_id == 0
            || code.is_empty()
            || code.len() > 10
            || code.bytes().any(|byte| !byte.is_ascii_digit())
            || code.parse::<u32>().ok().is_none_or(|value| value == 0)
        {
            return Err(EventHubError::InvalidEvent);
        }
        Ok(Self {
            account,
            group_id,
            message_id,
            operator_id,
            add,
            code,
            count,
            occurred_at,
        })
    }

    /// Returns the authenticated receiving account.
    #[must_use]
    pub const fn account(&self) -> &AccountIdentity {
        &self.account
    }

    /// Returns the numeric QQ group identifier.
    #[must_use]
    pub const fn group_id(&self) -> u64 {
        self.group_id
    }

    /// Returns the retained account-local message identifier.
    #[must_use]
    pub const fn message_id(&self) -> u32 {
        self.message_id
    }

    /// Returns the resolved numeric operator identifier.
    #[must_use]
    pub const fn operator_id(&self) -> u64 {
        self.operator_id
    }

    /// Returns whether the reaction was added rather than removed.
    #[must_use]
    pub const fn is_add(&self) -> bool {
        self.add
    }

    /// Returns the decimal reaction identifier.
    #[must_use]
    pub fn code(&self) -> &str {
        &self.code
    }

    /// Returns QQ's aggregate count after the change.
    #[must_use]
    pub const fn count(&self) -> u32 {
        self.count
    }

    /// Returns the QQ-supplied Unix event time in seconds, or zero when absent.
    #[must_use]
    pub const fn occurred_at(&self) -> u64 {
        self.occurred_at
    }
}
