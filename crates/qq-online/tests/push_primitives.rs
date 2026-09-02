//! Compiled Push primitive tests with neutral Profile routes.

use prost::Message;
use qq_online::{OnlineSyncState, PushOutcome, PushProcessor};
use qq_profile::{PushBehavior, PushPlanEntry};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[derive(Clone, Copy, PartialEq, Message)]
struct Pair {
    #[prost(uint32, tag = "1")]
    first: u32,
    #[prost(uint64, tag = "2")]
    second: u64,
}

#[derive(Clone, PartialEq, Message)]
struct Notice {
    #[prost(uint32, tag = "1")]
    account: u32,
    #[prost(string, tag = "3")]
    detail: String,
    #[prost(string, tag = "4")]
    title: String,
    #[prost(int32, tag = "5")]
    reason: i32,
}

#[test]
fn echo_and_pair_ack_are_profile_routed_and_bounded() -> TestResult {
    let mut processor = PushProcessor::default();
    let mut sync = OnlineSyncState::default();
    let echo = entry(PushBehavior::EchoBody, Some("push.ack"), 3)?;
    assert_eq!(
        processor.apply(&echo, b"abc", &mut sync, 1, 128)?,
        PushOutcome::Ack {
            route: "push.ack".to_owned(),
            body: b"abc".to_vec(),
        }
    );
    assert!(processor.apply(&echo, b"abcd", &mut sync, 1, 128).is_err());

    let pair = Pair {
        first: 7,
        second: 9,
    };
    let ack = entry(PushBehavior::ProtobufPairAck, Some("push.pair.ack"), 128)?;
    let PushOutcome::Ack { body, .. } =
        processor.apply(&ack, &pair.encode_to_vec(), &mut sync, 1, 128)?
    else {
        return Err("expected pair acknowledgement".into());
    };
    assert_eq!(Pair::decode(body.as_slice())?, pair);
    assert!(
        processor
            .apply(
                &ack,
                &Pair {
                    first: 8,
                    second: 9,
                }
                .encode_to_vec(),
                &mut sync,
                1,
                128,
            )
            .is_err()
    );
    Ok(())
}

#[test]
fn protective_notice_projects_only_bounded_public_fields() -> TestResult {
    let mut processor = PushProcessor::default();
    let mut sync = OnlineSyncState::default();
    let body = Notice {
        account: 42,
        detail: "detail".to_owned(),
        title: "title".to_owned(),
        reason: 5,
    }
    .encode_to_vec();
    let outcome = processor.apply(
        &entry(PushBehavior::ProtectiveOffline, None, 4_096)?,
        &body,
        &mut sync,
        1,
        128,
    )?;
    let PushOutcome::ProtectiveOffline(notice) = outcome else {
        return Err("expected protective offline".into());
    };
    assert_eq!(notice.account, 42);
    assert_eq!(notice.reason_code, 5);
    assert_eq!(notice.title, "title");
    Ok(())
}

fn entry(
    behavior: PushBehavior,
    response: Option<&str>,
    maximum: u32,
) -> Result<PushPlanEntry, qq_profile::PushPlanError> {
    PushPlanEntry::new(
        "push.input".to_owned(),
        behavior,
        response.map(str::to_owned),
        if behavior == PushBehavior::ProtobufPairAck {
            7
        } else {
            0
        },
        maximum,
    )
}
