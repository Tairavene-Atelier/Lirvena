//! Closed profile-negotiation outcome tests.

use ceylith_protocol::{ProfileId, ProfileOutcome, decode_profile_outcome, proto};

fn decision(status: proto::ProfileStatus) -> proto::ProfileDecision {
    proto::ProfileDecision {
        status: status as i32,
        profile_id: vec![8; 16],
        required_runtime_abi: 0,
        manifest: Vec::new(),
        manifest_digest: Vec::new(),
        manifest_signature: Vec::new(),
        expires_at_ms: 0,
        policy_epoch: 9,
    }
}

#[test]
fn all_profile_negotiation_outcomes_are_closed() -> Result<(), Box<dyn std::error::Error>> {
    let mut ready = decision(proto::ProfileStatus::Ready);
    ready.manifest = vec![1, 2, 3];
    ready.manifest_digest = vec![2; 32];
    ready.manifest_signature = vec![3; 64];
    ready.expires_at_ms = 10;
    assert!(matches!(
        decode_profile_outcome(&ready)?,
        ProfileOutcome::Ready(_)
    ));

    let mut upgrade = decision(proto::ProfileStatus::ClientUpgradeRequired);
    upgrade.required_runtime_abi = 7;
    assert_eq!(
        decode_profile_outcome(&upgrade)?,
        ProfileOutcome::ClientUpgradeRequired {
            profile_id: ProfileId::from_bytes([8; 16]),
            required_runtime_abi: 7,
            policy_epoch: 9,
        }
    );

    let unavailable = decision(proto::ProfileStatus::Unavailable);
    assert_eq!(
        decode_profile_outcome(&unavailable)?,
        ProfileOutcome::Unavailable {
            profile_id: ProfileId::from_bytes([8; 16]),
            policy_epoch: 9,
        }
    );
    Ok(())
}

#[test]
fn non_ready_outcome_cannot_smuggle_manifest_material() {
    let mut unavailable = decision(proto::ProfileStatus::Unavailable);
    unavailable.manifest = vec![1];
    assert!(decode_profile_outcome(&unavailable).is_err());
}
