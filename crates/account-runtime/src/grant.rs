use std::collections::{BTreeMap, BTreeSet};

use crate::AccountLocalId;

/// User-selected capability policy for one account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AccountGrantMode {
    /// Always use the anonymous Public capability.
    Public,
    /// Refuse startup unless a complete Full capability is available.
    RequireGrant,
    /// Prefer Full, but permit an explicitly warned Public startup fallback.
    AllowPublicFallback,
}

/// One configured account presented to the installation-wide policy planner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AccountGrantRequest {
    local_id: AccountLocalId,
    mode: AccountGrantMode,
}

impl AccountGrantRequest {
    /// Creates an account grant request.
    #[must_use]
    pub const fn new(local_id: AccountLocalId, mode: AccountGrantMode) -> Self {
        Self { local_id, mode }
    }

    /// Returns the opaque installation-local account identifier.
    #[must_use]
    pub const fn local_id(self) -> AccountLocalId {
        self.local_id
    }

    /// Returns the configured grant mode.
    #[must_use]
    pub const fn mode(self) -> AccountGrantMode {
        self.mode
    }
}

/// Installation-wide capability currently authenticated by Ceylith.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GrantAvailability {
    /// No authenticated Full grant is available.
    PublicOnly,
    /// Full is available with a finite simultaneous-account limit.
    BoundedFull {
        /// Maximum simultaneous Full accounts.
        max_accounts: u32,
    },
    /// Full is available without an account limit.
    UnboundedFull,
}

/// Capability selected for one account generation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AssignedRealm {
    /// Anonymous Public capability.
    Public,
    /// Complete authenticated Full capability.
    Full,
}

/// Complete, deterministic startup plan for an installation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GrantPlan {
    assignments: BTreeMap<AccountLocalId, AssignedRealm>,
    public_fallbacks: BTreeSet<AccountLocalId>,
}

impl GrantPlan {
    /// Returns the selected capability for an account.
    #[must_use]
    pub fn assigned_realm(&self, local_id: AccountLocalId) -> Option<AssignedRealm> {
        self.assignments.get(&local_id).copied()
    }

    /// Accounts that require a prominent Public-fallback warning at startup.
    #[must_use]
    pub const fn public_fallbacks(&self) -> &BTreeSet<AccountLocalId> {
        &self.public_fallbacks
    }

    /// Accounts that must enter `ProtectiveOffline` when the Full grant is revoked.
    ///
    /// This list never includes Public accounts, including startup fallbacks. Callers must close
    /// each listed QQ transport before considering a later Public generation.
    #[must_use]
    pub fn protective_offline_on_revocation(&self) -> BTreeSet<AccountLocalId> {
        self.assignments
            .iter()
            .filter_map(|(local_id, realm)| (*realm == AssignedRealm::Full).then_some(*local_id))
            .collect()
    }
}

/// Installation-wide grant planning rejection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GrantPlanError {
    /// The same installation-local account was configured more than once.
    DuplicateAccount {
        /// Repeated installation-local account identifier.
        account: AccountLocalId,
    },
    /// These accounts require Full while only Public is available.
    GrantRequired {
        /// Every account configured as `RequireGrant`.
        accounts: BTreeSet<AccountLocalId>,
    },
    /// More accounts requested Full than the authenticated grant permits.
    FullQuotaExceeded {
        /// Authenticated simultaneous Full-account limit.
        limit: u32,
        /// Every non-Public account participating in the conflict.
        accounts: BTreeSet<AccountLocalId>,
    },
}

impl core::fmt::Display for GrantPlanError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::DuplicateAccount { .. } => {
                formatter.write_str("account grant configuration contains a duplicate account")
            }
            Self::GrantRequired { .. } => {
                formatter.write_str("one or more accounts require an unavailable Full grant")
            }
            Self::FullQuotaExceeded { .. } => {
                formatter.write_str("configured Full accounts exceed the authenticated quota")
            }
        }
    }
}

impl std::error::Error for GrantPlanError {}

/// Plans every configured account together without silently choosing quota winners.
///
/// # Errors
///
/// Returns all conflicting accounts for missing grants or quota overflow, or the duplicated
/// installation-local identifier. A bounded Full grant with a zero limit is rejected as an
/// overflow rather than interpreted as unlimited.
pub fn plan_account_grants(
    requests: impl IntoIterator<Item = AccountGrantRequest>,
    availability: GrantAvailability,
) -> Result<GrantPlan, GrantPlanError> {
    let requests = collect_unique(requests)?;
    let full_candidates = requests
        .iter()
        .filter_map(|(local_id, mode)| (*mode != AccountGrantMode::Public).then_some(*local_id))
        .collect::<BTreeSet<_>>();

    match availability {
        GrantAvailability::PublicOnly => plan_public_only(&requests),
        GrantAvailability::BoundedFull { max_accounts }
            if full_candidates.len() > max_accounts as usize =>
        {
            Err(GrantPlanError::FullQuotaExceeded {
                limit: max_accounts,
                accounts: full_candidates,
            })
        }
        GrantAvailability::BoundedFull { .. } | GrantAvailability::UnboundedFull => {
            Ok(plan_with_full(requests))
        }
    }
}

fn collect_unique(
    requests: impl IntoIterator<Item = AccountGrantRequest>,
) -> Result<BTreeMap<AccountLocalId, AccountGrantMode>, GrantPlanError> {
    let mut unique = BTreeMap::new();
    for request in requests {
        if unique.insert(request.local_id(), request.mode()).is_some() {
            return Err(GrantPlanError::DuplicateAccount {
                account: request.local_id(),
            });
        }
    }
    Ok(unique)
}

fn plan_public_only(
    requests: &BTreeMap<AccountLocalId, AccountGrantMode>,
) -> Result<GrantPlan, GrantPlanError> {
    let required = requests
        .iter()
        .filter_map(|(local_id, mode)| {
            (*mode == AccountGrantMode::RequireGrant).then_some(*local_id)
        })
        .collect::<BTreeSet<_>>();
    if !required.is_empty() {
        return Err(GrantPlanError::GrantRequired { accounts: required });
    }

    let public_fallbacks = requests
        .iter()
        .filter_map(|(local_id, mode)| {
            (*mode == AccountGrantMode::AllowPublicFallback).then_some(*local_id)
        })
        .collect();
    Ok(GrantPlan {
        assignments: requests
            .keys()
            .map(|local_id| (*local_id, AssignedRealm::Public))
            .collect(),
        public_fallbacks,
    })
}

fn plan_with_full(requests: BTreeMap<AccountLocalId, AccountGrantMode>) -> GrantPlan {
    GrantPlan {
        assignments: requests
            .into_iter()
            .map(|(local_id, mode)| {
                let realm = match mode {
                    AccountGrantMode::Public => AssignedRealm::Public,
                    AccountGrantMode::RequireGrant | AccountGrantMode::AllowPublicFallback => {
                        AssignedRealm::Full
                    }
                };
                (local_id, realm)
            })
            .collect(),
        public_fallbacks: BTreeSet::new(),
    }
}
