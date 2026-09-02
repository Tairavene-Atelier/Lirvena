use qq_message::{MemberDecreaseKind, MemberIncreaseKind};

use crate::AccountIdentity;

/// One authenticated group notice after Linux NT UIDs have been resolved to numeric QQ IDs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedGroupNotice {
    account: AccountIdentity,
    group_id: u64,
    user_id: u64,
    operator_id: Option<u64>,
    kind: ResolvedGroupNoticeKind,
    occurred_at: u64,
}

impl ResolvedGroupNotice {
    /// Creates an adapter-neutral resolved notice.
    ///
    /// # Errors
    ///
    /// Returns an error for missing group or member identity.
    pub fn new(
        account: AccountIdentity,
        group_id: u64,
        user_id: u64,
        operator_id: Option<u64>,
        kind: ResolvedGroupNoticeKind,
        occurred_at: u64,
    ) -> Result<Self, crate::EventHubError> {
        if group_id == 0 || user_id == 0 || operator_id == Some(0) {
            return Err(crate::EventHubError::InvalidEvent);
        }
        Ok(Self {
            account,
            group_id,
            user_id,
            operator_id,
            kind,
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

    /// Returns the affected member identifier.
    #[must_use]
    pub const fn user_id(&self) -> u64 {
        self.user_id
    }

    /// Returns the inviter or operator when QQ evidence resolved it.
    #[must_use]
    pub const fn operator_id(&self) -> Option<u64> {
        self.operator_id
    }

    /// Returns the evidence-backed notice kind.
    #[must_use]
    pub const fn kind(&self) -> ResolvedGroupNoticeKind {
        self.kind
    }

    /// Returns the QQ-supplied Unix event time in seconds, or zero when absent.
    #[must_use]
    pub const fn occurred_at(&self) -> u64 {
        self.occurred_at
    }
}

/// Adapter-neutral group notice semantics supported by current evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResolvedGroupNoticeKind {
    /// Administrator status was granted.
    AdministratorSet,
    /// Administrator status was removed.
    AdministratorUnset,
    /// One member entered the group.
    MemberIncrease(MemberIncreaseKind),
    /// One member left or was removed.
    MemberDecrease(MemberDecreaseKind),
}
