//! Installation-wide grant planning contract tests.

use std::collections::BTreeSet;

use account_runtime::{
    AccountGrantMode, AccountGrantRequest, AccountLocalId, AssignedRealm, GrantAvailability,
    GrantPlanError, plan_account_grants,
};

#[test]
fn public_only_rejects_every_require_grant_account_together() {
    let public = account(1, AccountGrantMode::Public);
    let first_required = account(2, AccountGrantMode::RequireGrant);
    let fallback = account(3, AccountGrantMode::AllowPublicFallback);
    let second_required = account(4, AccountGrantMode::RequireGrant);

    let result = plan_account_grants(
        [public, first_required, fallback, second_required],
        GrantAvailability::PublicOnly,
    );
    assert_eq!(
        result,
        Err(GrantPlanError::GrantRequired {
            accounts: BTreeSet::from([first_required.local_id(), second_required.local_id()]),
        })
    );
}

#[test]
fn public_fallback_is_explicitly_reported() -> Result<(), GrantPlanError> {
    let public = account(1, AccountGrantMode::Public);
    let fallback = account(2, AccountGrantMode::AllowPublicFallback);
    let plan = plan_account_grants([public, fallback], GrantAvailability::PublicOnly)?;

    assert_eq!(
        plan.assigned_realm(public.local_id()),
        Some(AssignedRealm::Public)
    );
    assert_eq!(
        plan.assigned_realm(fallback.local_id()),
        Some(AssignedRealm::Public)
    );
    assert_eq!(
        plan.public_fallbacks(),
        &BTreeSet::from([fallback.local_id()])
    );
    assert!(plan.protective_offline_on_revocation().is_empty());
    Ok(())
}

#[test]
fn bounded_full_never_selects_quota_winners() {
    let first = account(1, AccountGrantMode::RequireGrant);
    let second = account(2, AccountGrantMode::AllowPublicFallback);
    let result = plan_account_grants(
        [first, second],
        GrantAvailability::BoundedFull { max_accounts: 1 },
    );

    assert_eq!(
        result,
        Err(GrantPlanError::FullQuotaExceeded {
            limit: 1,
            accounts: BTreeSet::from([first.local_id(), second.local_id()]),
        })
    );
}

#[test]
fn full_assignments_share_behavior_and_revoke_together() -> Result<(), GrantPlanError> {
    let public = account(1, AccountGrantMode::Public);
    let community_account = account(2, AccountGrantMode::RequireGrant);
    let full_account = account(3, AccountGrantMode::AllowPublicFallback);
    let plan = plan_account_grants(
        [public, community_account, full_account],
        GrantAvailability::UnboundedFull,
    )?;

    assert_eq!(
        plan.assigned_realm(community_account.local_id()),
        Some(AssignedRealm::Full)
    );
    assert_eq!(
        plan.assigned_realm(full_account.local_id()),
        Some(AssignedRealm::Full)
    );
    assert_eq!(
        plan.protective_offline_on_revocation(),
        BTreeSet::from([community_account.local_id(), full_account.local_id()])
    );
    Ok(())
}

#[test]
fn duplicate_account_configuration_is_rejected() {
    let local_id = AccountLocalId::from_bytes([9; 16]);
    let result = plan_account_grants(
        [
            AccountGrantRequest::new(local_id, AccountGrantMode::Public),
            AccountGrantRequest::new(local_id, AccountGrantMode::RequireGrant),
        ],
        GrantAvailability::UnboundedFull,
    );

    assert_eq!(
        result,
        Err(GrantPlanError::DuplicateAccount { account: local_id })
    );
}

const fn account(marker: u8, mode: AccountGrantMode) -> AccountGrantRequest {
    AccountGrantRequest::new(AccountLocalId::from_bytes([marker; 16]), mode)
}
