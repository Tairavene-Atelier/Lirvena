//! Installation-signature and grant-isolation tests for Community telemetry.

use ceylith_client::{CommunityTelemetrySigner, CommunityTelemetrySpec};
use ceylith_protocol::{
    AccountChurnBucket, ActiveDurationBucket, Digest32, GroupCountBucket, MessageCountBucket,
    ProfileId, SessionAdmission, SessionId, TelemetryReportId, proto, telemetry_signing_transcript,
};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};

#[test]
fn community_report_is_installation_signed_and_full_is_rejected()
-> Result<(), Box<dyn std::error::Error>> {
    let seed = [7_u8; 32];
    let signer = CommunityTelemetrySigner::from_seed(seed);
    let frame = signer.report(
        &admission(ceylith_protocol::proto::GrantClass::Community)?,
        spec(),
    )?;
    let Some(proto::inner_frame::Body::CommunityTelemetryReport(report)) = frame.body else {
        return Err(std::io::Error::other("missing telemetry report").into());
    };
    let signature: [u8; 64] = report.installation_signature.as_slice().try_into()?;
    VerifyingKey::from(&ed25519_dalek::SigningKey::from_bytes(&seed)).verify(
        &telemetry_signing_transcript(&report)?,
        &Signature::from_bytes(&signature),
    )?;
    assert!(
        signer
            .report(
                &admission(ceylith_protocol::proto::GrantClass::Full)?,
                spec(),
            )
            .is_err()
    );
    Ok(())
}

fn admission(
    grant_class: proto::GrantClass,
) -> Result<SessionAdmission, ceylith_protocol::CodecError> {
    ceylith_protocol::decode_session_welcome(&proto::SessionWelcome {
        session_id: SessionId::from_bytes([1; 16]).as_bytes().to_vec(),
        runtime_lease: vec![2; 32],
        lease_expires_at_ms: 2_000,
        grant_class: grant_class as i32,
        max_full_accounts: 2,
        max_active_installations: 2,
        max_registered_installations: 2,
        server_time_ms: 1_000,
        policy_epoch: 1,
        accepted_contracts: vec![ceylith_protocol::CURRENT_INNER_CONTRACT],
    })
}

fn spec() -> CommunityTelemetrySpec {
    CommunityTelemetrySpec {
        report_id: TelemetryReportId::from_bytes([3; 16]),
        utc_day: 20_000,
        group_count: GroupCountBucket::OneToFive,
        messages_received: MessageCountBucket::OneToTwenty,
        messages_sent: MessageCountBucket::Zero,
        active_duration: ActiveDurationBucket::OneToFourHours,
        profile_id: ProfileId::from_bytes([4; 16]),
        profile_manifest_digest: Digest32::from_bytes([5; 32]),
        build_digest: Digest32::from_bytes([6; 32]),
        platform: 1,
        architecture: 1,
        account_churn: AccountChurnBucket::One,
        generated_at_ms: 1_700_000_000_000,
    }
}
