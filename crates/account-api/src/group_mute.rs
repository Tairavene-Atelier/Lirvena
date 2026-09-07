use crate::AccountIdentity;

/// One authenticated group mute after current-generation UIDs were resolved.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedGroupMute {
    account: AccountIdentity,
    group_id: u64,
    operator_id: Option<u64>,
    target_id: Option<u64>,
    duration: u32,
    occurred_at: u64,
}

impl ResolvedGroupMute {
    /// Creates an adapter-neutral mute notice.
    ///
    /// # Errors
    ///
    /// Returns an error for a missing group or a zero resolved identity.
    pub fn new(
        account: AccountIdentity,
        group_id: u64,
        operator_id: Option<u64>,
        target_id: Option<u64>,
        duration: u32,
        occurred_at: u64,
    ) -> Result<Self, crate::EventHubError> {
        if group_id == 0 || operator_id == Some(0) || target_id == Some(0) {
            return Err(crate::EventHubError::InvalidEvent);
        }
        Ok(Self {
            account,
            group_id,
            operator_id,
            target_id,
            duration,
            occurred_at,
        })
    }

    /// Returns the receiving account identity.
    #[must_use]
    pub const fn account(&self) -> &AccountIdentity {
        &self.account
    }
    /// Returns the numeric QQ group identifier.
    #[must_use]
    pub const fn group_id(&self) -> u64 {
        self.group_id
    }
    /// Returns the resolved operator, when QQ supplied one.
    #[must_use]
    pub const fn operator_id(&self) -> Option<u64> {
        self.operator_id
    }
    /// Returns the muted member, or `None` for a whole-group change.
    #[must_use]
    pub const fn target_id(&self) -> Option<u64> {
        self.target_id
    }
    /// Returns the QQ duration in seconds; zero denotes unmute.
    #[must_use]
    pub const fn duration(&self) -> u32 {
        self.duration
    }
    /// Returns the QQ-supplied Unix event time in seconds.
    #[must_use]
    pub const fn occurred_at(&self) -> u64 {
        self.occurred_at
    }
}
