//! Community telemetry bucketing, signature transcript and wire tests.

use ceylith_protocol::{
    AccountChurnBucket, ActiveDurationBucket, CURRENT_INNER_CONTRACT, GroupCountBucket,
    MessageCountBucket, WireLimits, decode_inner_frame, encode_inner_frame, proto,
    telemetry_signing_transcript,
};

#[test]
fn exact_counts_are_reduced_to_the_frozen_buckets() {
    assert_eq!(GroupCountBucket::from_count(0), GroupCountBucket::Zero);
    assert_eq!(
        GroupCountBucket::from_count(501),
        GroupCountBucket::OverFiveHundred
    );
    assert_eq!(
        MessageCountBucket::from_count(10_001),
        MessageCountBucket::OverTenThousand
    );
    assert_eq!(
        ActiveDurationBucket::from_milliseconds(1),
        ActiveDurationBucket::UnderOneHour
    );
    assert_eq!(
        AccountChurnBucket::from_count(8),
        AccountChurnBucket::EightOrMore
    );
}

#[test]
fn report_signature_is_canonical_and_wire_is_closed() -> Result<(), Box<dyn std::error::Error>> {
    let mut report = report();
    let first = telemetry_signing_transcript(&report)?;
    report.installation_signature = vec![9; 64];
    assert_eq!(telemetry_signing_transcript(&report)?, first);
    report.installation_signature = vec![9; 64];
    let frame = proto::InnerFrame {
        contract: CURRENT_INNER_CONTRACT,
        body: Some(proto::inner_frame::Body::CommunityTelemetryReport(report)),
    };
    let encoded = encode_inner_frame(&frame, WireLimits::default())?;
    assert_eq!(decode_inner_frame(&encoded, WireLimits::default())?, frame);
    Ok(())
}

#[test]
fn unspecified_bucket_and_invalid_signature_fail_closed() {
    let mut report = report();
    report.installation_signature = vec![1; 63];
    let frame = proto::InnerFrame {
        contract: CURRENT_INNER_CONTRACT,
        body: Some(proto::inner_frame::Body::CommunityTelemetryReport(report)),
    };
    assert!(encode_inner_frame(&frame, WireLimits::default()).is_err());
}

fn report() -> proto::CommunityTelemetryReport {
    proto::CommunityTelemetryReport {
        runtime_lease: vec![1; 32],
        report_id: vec![2; 16],
        utc_day: 20_000,
        group_count: GroupCountBucket::OneToFive.to_wire() as i32,
        messages_received: MessageCountBucket::OneToTwenty.to_wire() as i32,
        messages_sent: MessageCountBucket::Zero.to_wire() as i32,
        active_duration: ActiveDurationBucket::OneToFourHours.to_wire() as i32,
        profile_id: vec![3; 16],
        profile_manifest_digest: vec![4; 32],
        build_digest: vec![5; 32],
        platform: 1,
        architecture: 1,
        account_churn: AccountChurnBucket::One.to_wire() as i32,
        generated_at_ms: 1_700_000_000_000,
        installation_signature: Vec::new(),
    }
}
