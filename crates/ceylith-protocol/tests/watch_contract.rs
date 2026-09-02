//! Public Watch request and event contract tests.

use ceylith_protocol::{
    CURRENT_INNER_CONTRACT, CodecError, GrantClass, RenewalState, WatchEventKind, WatchOutcome,
    WireLimits, decode_inner_frame, decode_watch_response, encode_inner_frame, proto,
};

#[test]
fn grant_event_round_trips_as_a_typed_snapshot() -> Result<(), CodecError> {
    let frame = event_frame(8, proto::WatchEventKind::RenewalPaused, Some(snapshot()));
    let encoded = encode_inner_frame(&frame, WireLimits::default())?;
    let decoded = decode_inner_frame(&encoded, WireLimits::default())?;
    let WatchOutcome::Event(event) = decode_watch_response(&decoded, 7)? else {
        return Err(CodecError::InvalidField);
    };

    assert_eq!(event.cursor(), 8);
    assert_eq!(event.kind(), WatchEventKind::RenewalPaused);
    assert_eq!(event.reason_code(), 11);
    let grant = event.grant().ok_or(CodecError::InvalidField)?;
    assert_eq!(grant.grant_class(), GrantClass::Community);
    assert_eq!(grant.max_full_accounts(), 3);
    assert_eq!(grant.max_active_installations(), 2);
    assert_eq!(grant.expires_at_ms(), 9_000);
    assert_eq!(grant.renewal_state(), RenewalState::Paused);
    assert_eq!(grant.policy_epoch(), 4);
    Ok(())
}

#[test]
fn grant_events_require_snapshots_and_advancing_cursors() {
    let missing = event_frame(8, proto::WatchEventKind::GrantRevoked, None);
    assert!(encode_inner_frame(&missing, WireLimits::default()).is_err());

    let mut revoked = snapshot();
    revoked.renewal_state = proto::RenewalState::Revoked as i32;
    let valid = event_frame(8, proto::WatchEventKind::GrantRevoked, Some(revoked));
    assert!(decode_watch_response(&valid, 8).is_err());

    let contradictory = event_frame(9, proto::WatchEventKind::GrantRevoked, Some(snapshot()));
    assert!(encode_inner_frame(&contradictory, WireLimits::default()).is_err());
}

#[test]
fn profile_events_reject_unrelated_grant_snapshots() {
    let contradictory = event_frame(8, proto::WatchEventKind::ProfileChanged, Some(snapshot()));
    assert!(encode_inner_frame(&contradictory, WireLimits::default()).is_err());
}

#[test]
fn idle_response_preserves_the_requested_cursor() -> Result<(), CodecError> {
    let frame = proto::InnerFrame {
        contract: CURRENT_INNER_CONTRACT,
        body: Some(proto::inner_frame::Body::GenericResult(
            proto::GenericResult {
                accepted: true,
                code: 1,
                payload: Vec::new(),
            },
        )),
    };
    assert_eq!(
        decode_watch_response(&frame, 19)?,
        WatchOutcome::Idle { cursor: 19 }
    );
    Ok(())
}

fn event_frame(
    cursor: u64,
    kind: proto::WatchEventKind,
    grant: Option<proto::WatchGrantSnapshot>,
) -> proto::InnerFrame {
    proto::InnerFrame {
        contract: CURRENT_INNER_CONTRACT,
        body: Some(proto::inner_frame::Body::WatchEvent(proto::WatchEvent {
            cursor,
            kind: kind as i32,
            occurred_at_ms: 1_000,
            account_slot_id: Vec::new(),
            reason_code: 11,
            payload: Vec::new(),
            grant,
        })),
    }
}

fn snapshot() -> proto::WatchGrantSnapshot {
    proto::WatchGrantSnapshot {
        grant_class: proto::GrantClass::Community as i32,
        max_full_accounts: 3,
        max_active_installations: 2,
        expires_at_ms: 9_000,
        renewal_state: proto::RenewalState::Paused as i32,
        policy_epoch: 4,
    }
}
