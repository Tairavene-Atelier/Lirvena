use account_runtime::{AccountLocalId, GrantPlan, ProtectiveReason};
use ceylith_protocol::{GrantClass, WatchEvent, WatchEventKind};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ContinuityAction {
    Continue,
    Protect(ProtectiveReason),
}

pub(super) fn classify(
    event: &WatchEvent,
    account: AccountLocalId,
    plan: &GrantPlan,
) -> ContinuityAction {
    if event
        .account_slot_id()
        .is_some_and(|scope| scope.as_bytes() != account.as_bytes())
    {
        return ContinuityAction::Continue;
    }
    match event.kind() {
        WatchEventKind::GrantRevoked => {
            ContinuityAction::Protect(ProtectiveReason::GrantUnavailable)
        }
        WatchEventKind::ProfileChanged | WatchEventKind::Maintenance => {
            ContinuityAction::Protect(ProtectiveReason::ProfileUnavailable)
        }
        WatchEventKind::QuotaChanged if quota_exceeded(event, plan) => {
            ContinuityAction::Protect(ProtectiveReason::GrantUnavailable)
        }
        WatchEventKind::GrantExpiring
        | WatchEventKind::RenewalPaused
        | WatchEventKind::QuotaChanged
        | WatchEventKind::PolicyChanged
        | WatchEventKind::GrantRestored => ContinuityAction::Continue,
    }
}

fn quota_exceeded(event: &WatchEvent, plan: &GrantPlan) -> bool {
    let Some(grant) = event.grant() else {
        return true;
    };
    let required = plan.protective_offline_on_revocation().len();
    match grant.grant_class() {
        GrantClass::Public => required != 0,
        GrantClass::Full if grant.max_full_accounts() == 0 => false,
        GrantClass::Community | GrantClass::Full => required > grant.max_full_accounts() as usize,
    }
}

#[cfg(test)]
mod tests {
    use account_runtime::{
        AccountGrantMode, AccountGrantRequest, GrantAvailability, plan_account_grants,
    };
    use ceylith_protocol::{CURRENT_INNER_CONTRACT, WatchOutcome, decode_watch_response, proto};

    use super::{ContinuityAction, classify};
    use account_runtime::{AccountLocalId, ProtectiveReason};

    #[test]
    fn revocation_protects_full_account() -> Result<(), Box<dyn std::error::Error>> {
        let account = AccountLocalId::from_bytes([1; 16]);
        let plan = plan_account_grants(
            [AccountGrantRequest::new(
                account,
                AccountGrantMode::RequireGrant,
            )],
            GrantAvailability::BoundedFull { max_accounts: 1 },
        )?;
        let event = event(proto::WatchEventKind::GrantRevoked, account, 1, true)?;
        assert_eq!(
            classify(&event, account, &plan),
            ContinuityAction::Protect(ProtectiveReason::GrantUnavailable)
        );
        Ok(())
    }

    #[test]
    fn event_for_another_account_is_ignored() -> Result<(), Box<dyn std::error::Error>> {
        let account = AccountLocalId::from_bytes([1; 16]);
        let plan = plan_account_grants(
            [AccountGrantRequest::new(
                account,
                AccountGrantMode::RequireGrant,
            )],
            GrantAvailability::BoundedFull { max_accounts: 1 },
        )?;
        let event = event(
            proto::WatchEventKind::GrantRevoked,
            AccountLocalId::from_bytes([2; 16]),
            1,
            true,
        )?;
        assert_eq!(classify(&event, account, &plan), ContinuityAction::Continue);
        Ok(())
    }

    fn event(
        kind: proto::WatchEventKind,
        account: AccountLocalId,
        max_full_accounts: u32,
        revoked: bool,
    ) -> Result<ceylith_protocol::WatchEvent, Box<dyn std::error::Error>> {
        let frame = proto::InnerFrame {
            contract: CURRENT_INNER_CONTRACT,
            body: Some(proto::inner_frame::Body::WatchEvent(proto::WatchEvent {
                cursor: 1,
                kind: kind as i32,
                occurred_at_ms: 1,
                account_slot_id: account.as_bytes().to_vec(),
                reason_code: 1,
                payload: Vec::new(),
                grant: Some(proto::WatchGrantSnapshot {
                    grant_class: proto::GrantClass::Community as i32,
                    max_full_accounts,
                    max_active_installations: 1,
                    expires_at_ms: 2,
                    renewal_state: if revoked {
                        proto::RenewalState::Revoked as i32
                    } else {
                        proto::RenewalState::Current as i32
                    },
                    policy_epoch: 1,
                }),
            })),
        };
        let WatchOutcome::Event(event) = decode_watch_response(&frame, 0)? else {
            return Err(std::io::Error::other("test Watch event decoded as idle").into());
        };
        Ok(event)
    }
}
