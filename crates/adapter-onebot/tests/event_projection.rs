//! Evidence-preserving `OneBot` message-event projection contracts.

use account_api::{
    AccountEvent, AccountIdentity, FriendRequestReference, GroupRequestKind, GroupRequestReference,
    InboundMessage, ResolvedFriendRequest, ResolvedGroupNotice, ResolvedGroupNoticeKind,
    ResolvedGroupReaction, ResolvedGroupRequest,
};
use account_runtime::AccountLocalId;
use adapter_onebot::{IdFormat, project_account_event, project_message_record};
use prost::Message;
use qq_message::{
    MemberDecreaseKind, MemberIncreaseKind, MessageDecoder, MessageDisposition, OutboundSegment,
    RichTextMessage, SendMessageInput, SendTextTarget, decode_rich_text, encode_message,
};
use serde_json::json;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn group_request_projects_actionable_opaque_flag() -> TestResult {
    let reference = GroupRequestReference::new(77, 1, 12_345)?;
    let event = AccountEvent::GroupRequest(Box::new(ResolvedGroupRequest::new(
        identity()?,
        reference,
        GroupRequestKind::Join,
        42,
        None,
        "hello".to_owned(),
        1_800_000_000,
    )?));
    let projected = project_account_event(&event, IdFormat::String)?.ok_or("missing request")?;
    assert_eq!(
        projected,
        json!({
            "time": 1_800_000_000,
            "self_id": "10001",
            "post_type": "request",
            "request_type": "group",
            "sub_type": "add",
            "group_id": "12345",
            "user_id": "42",
            "comment": "hello",
            "flag": reference.flag(),
        })
    );
    assert_eq!(
        GroupRequestReference::parse(projected["flag"].as_str().ok_or("flag")?)?,
        reference
    );
    Ok(())
}

#[test]
fn friend_request_projects_standard_actionable_fields() -> Result<(), Box<dyn std::error::Error>> {
    let reference = FriendRequestReference::new("u_friend".to_owned())?;
    let event = AccountEvent::FriendRequest(Box::new(ResolvedFriendRequest::new(
        identity()?,
        reference.clone(),
        42,
        "hello".to_owned(),
        99,
    )?));
    let projected = project_account_event(&event, IdFormat::String)?.ok_or("missing event")?;
    assert_eq!(projected["post_type"], "request");
    assert_eq!(projected["request_type"], "friend");
    assert_eq!(projected["user_id"], "42");
    assert_eq!(projected["flag"], reference.flag());
    Ok(())
}

#[test]
fn member_invitation_request_keeps_add_semantics_and_inviter_extension() -> TestResult {
    let reference = GroupRequestReference::new(78, 22, 12_345)?;
    let event = AccountEvent::GroupRequest(Box::new(ResolvedGroupRequest::new(
        identity()?,
        reference,
        GroupRequestKind::Invitation,
        42,
        Some(43),
        String::new(),
        1_800_000_001,
    )?));
    let projected = project_account_event(&event, IdFormat::Number)?.ok_or("missing request")?;
    assert_eq!(projected["sub_type"], "add");
    assert_eq!(projected["user_id"], 42);
    assert_eq!(projected["invitor_id"], 43);
    Ok(())
}

#[derive(Clone, PartialEq, Message)]
struct PushBody {
    #[prost(message, optional, tag = "1")]
    response: Option<Response>,
    #[prost(message, optional, tag = "2")]
    content: Option<Content>,
    #[prost(message, optional, tag = "3")]
    body: Option<Body>,
}

#[derive(Clone, PartialEq, Message)]
struct Response {
    #[prost(uint32, tag = "1")]
    from_uin: u32,
    #[prost(string, optional, tag = "2")]
    from_uid: Option<String>,
    #[prost(uint32, tag = "5")]
    to_uin: u32,
    #[prost(message, optional, tag = "7")]
    friend: Option<Friend>,
    #[prost(message, optional, tag = "8")]
    group: Option<Route>,
}

#[derive(Clone, PartialEq, Message)]
struct Friend {
    #[prost(string, optional, tag = "6")]
    name: Option<String>,
}

#[derive(Clone, PartialEq, Message)]
struct Route {
    #[prost(uint32, tag = "1")]
    group_uin: u32,
    #[prost(string, tag = "4")]
    member_name: String,
    #[prost(string, tag = "7")]
    group_name: String,
}

#[derive(Clone, Copy, PartialEq, Message)]
struct Content {
    #[prost(uint32, tag = "1")]
    message_type: u32,
    #[prost(uint64, optional, tag = "5")]
    sequence: Option<u64>,
    #[prost(int64, optional, tag = "6")]
    timestamp: Option<i64>,
}

#[derive(Clone, PartialEq, Message)]
struct Body {
    #[prost(bytes = "vec", optional, tag = "2")]
    content: Option<Vec<u8>>,
}

#[derive(Clone, PartialEq, Message)]
struct OutboundMessageFixture {
    #[prost(message, optional, tag = "3")]
    body: Option<OutboundBodyFixture>,
}

#[derive(Clone, PartialEq, Message)]
struct OutboundBodyFixture {
    #[prost(bytes = "vec", optional, tag = "1")]
    rich_text: Option<Vec<u8>>,
}

#[test]
fn group_message_includes_only_observed_sender_fields() -> TestResult {
    let event = event(82, Some((88, "member")), None)?;
    let projected = project_account_event(&event, IdFormat::String)?.ok_or("missing event")?;
    assert_eq!(projected["message_type"], "group");
    assert_eq!(projected["sub_type"], "normal");
    assert_eq!(projected["message_id"], "9");
    assert_eq!(projected["group_id"], "88");
    assert_eq!(
        projected["sender"],
        json!({"user_id": "42", "nickname": "member", "card": "member"})
    );
    assert!(projected["sender"].get("role").is_none());
    let AccountEvent::Message(message) = &event else {
        return Err("expected message".into());
    };
    let stored = project_message_record(message, IdFormat::Number)?;
    assert_eq!(stored["message_id"], 9);
    assert_eq!(stored["real_id"], 9);
    assert!(stored.get("post_type").is_none());
    Ok(())
}

#[test]
fn temporary_message_preserves_group_context_as_private_subtype() -> TestResult {
    let event = event(141, Some((88, "temporary member")), None)?;
    let projected = project_account_event(&event, IdFormat::Number)?.ok_or("missing event")?;
    assert_eq!(projected["message_type"], "private");
    assert_eq!(projected["sub_type"], "group");
    assert_eq!(projected["group_id"], 88);
    assert_eq!(projected["sender"]["user_id"], 42);
    Ok(())
}

#[test]
fn private_message_uses_the_observed_friend_name() -> TestResult {
    let event = event(166, None, Some("friend"))?;
    let projected = project_account_event(&event, IdFormat::Number)?.ok_or("missing event")?;
    assert_eq!(projected["message_type"], "private");
    assert_eq!(projected["sub_type"], "friend");
    assert_eq!(
        projected["sender"],
        json!({"user_id": 42, "nickname": "friend"})
    );
    Ok(())
}

#[test]
fn rich_segments_project_to_standard_onebot_shapes() -> TestResult {
    let encoded = encode_message(&SendMessageInput {
        target: SendTextTarget::Group { group_code: 88 },
        segments: &[
            OutboundSegment::Json("{\"app\":\"demo\"}"),
            OutboundSegment::Xml {
                body: "<msg/>",
                service_id: 35,
            },
            OutboundSegment::Xml {
                body: "<msg m_resid=\"forward-1\"/>",
                service_id: 35,
            },
            OutboundSegment::Poke {
                kind: 2,
                strength: 7,
            },
        ],
        client_sequence: 7,
        random: 8,
        unix_seconds: 9,
    })?;
    let rich = OutboundMessageFixture::decode(encoded.as_slice())?
        .body
        .and_then(|body| body.rich_text)
        .ok_or("missing rich text")?;
    let event = event_with_rich(
        82,
        Some((88, "member")),
        None,
        Some(decode_rich_text(&rich)?),
    )?;
    let projected = project_account_event(&event, IdFormat::String)?.ok_or("missing event")?;
    assert_eq!(
        projected["message"],
        json!([
            {"type": "json", "data": {"data": "{\"app\":\"demo\"}"}},
            {"type": "xml", "data": {"data": "<msg/>", "service_id": 35}},
            {"type": "forward", "data": {"id": "forward-1"}},
            {"type": "poke", "data": {"type": 2, "strength": 7, "id": -1}}
        ])
    );
    assert_eq!(
        projected["raw_message"],
        "[CQ:json][CQ:xml][CQ:forward,id=forward-1][CQ:poke,type=2,strength=7]"
    );
    Ok(())
}

#[test]
fn resolved_reply_uses_local_message_id_and_unresolved_reply_stays_honest() -> TestResult {
    let original = vec![vec![0x0a, 0x02, 0x68, 0x69]];
    let encoded = encode_message(&SendMessageInput {
        target: SendTextTarget::Group { group_code: 88 },
        segments: &[OutboundSegment::Reply {
            group: true,
            sequence: 7,
            message_uid: 9,
            sender_uin: 42,
            sender_uid: "u_source",
            timestamp: 10,
            elements: &original,
        }],
        client_sequence: 11,
        random: 12,
        unix_seconds: 13,
    })?;
    let rich = OutboundMessageFixture::decode(encoded.as_slice())?
        .body
        .and_then(|body| body.rich_text)
        .ok_or("missing rich text")?;
    let rich = decode_rich_text(&rich)?;
    let event = event_with_rich_and_replies(
        82,
        Some((88, "member")),
        None,
        Some(rich.clone()),
        vec![Some(55), None],
    )?;
    let projected = project_account_event(&event, IdFormat::Number)?.ok_or("missing event")?;
    assert_eq!(
        projected["message"][0],
        json!({"type": "reply", "data": {"id": 55}})
    );
    assert!(
        projected["raw_message"]
            .as_str()
            .is_some_and(|value| value.starts_with("[CQ:reply,id=55]"))
    );

    let unresolved =
        event_with_rich_and_replies(82, Some((88, "member")), None, Some(rich), vec![None, None])?;
    let projected =
        project_account_event(&unresolved, IdFormat::Number)?.ok_or("missing unresolved event")?;
    assert_eq!(projected["message"][0]["type"], "lirvena_unsupported");
    Ok(())
}

#[test]
fn group_notice_projects_only_resolved_numeric_identities() -> TestResult {
    let identity = identity()?;
    let increase = AccountEvent::GroupNotice(Box::new(ResolvedGroupNotice::new(
        identity.clone(),
        88,
        42,
        Some(43),
        ResolvedGroupNoticeKind::MemberIncrease(MemberIncreaseKind::Invite),
        1_800_000_000,
    )?));
    let projected =
        project_account_event(&increase, IdFormat::String)?.ok_or("missing increase event")?;
    assert_eq!(projected["notice_type"], "group_increase");
    assert_eq!(projected["sub_type"], "invite");
    assert_eq!(projected["operator_id"], "43");

    let decrease = AccountEvent::GroupNotice(Box::new(ResolvedGroupNotice::new(
        identity,
        88,
        10_001,
        None,
        ResolvedGroupNoticeKind::MemberDecrease(MemberDecreaseKind::KickMe),
        1_800_000_001,
    )?));
    let projected =
        project_account_event(&decrease, IdFormat::Number)?.ok_or("missing decrease event")?;
    assert_eq!(projected["notice_type"], "group_decrease");
    assert_eq!(projected["sub_type"], "kick_me");
    assert!(projected.get("operator_id").is_none());
    Ok(())
}

#[test]
fn unknown_group_notice_subtype_has_no_fabricated_projection() -> TestResult {
    let event = AccountEvent::GroupNotice(Box::new(ResolvedGroupNotice::new(
        identity()?,
        88,
        42,
        None,
        ResolvedGroupNoticeKind::MemberDecrease(MemberDecreaseKind::Unknown(999)),
        1_800_000_000,
    )?));
    assert!(project_account_event(&event, IdFormat::Number).is_err());
    Ok(())
}

#[test]
fn group_reaction_uses_lagrange_compatible_notice_shape() -> TestResult {
    let event = AccountEvent::GroupReaction(Box::new(ResolvedGroupReaction::new(
        identity()?,
        88,
        91,
        42,
        true,
        "14".to_owned(),
        3,
        1_800_000_000,
    )?));
    let projected =
        project_account_event(&event, IdFormat::String)?.ok_or("missing reaction event")?;
    assert_eq!(
        projected,
        json!({
            "time": 1_800_000_000,
            "self_id": "10001",
            "post_type": "notice",
            "notice_type": "group_msg_emoji_like",
            "group_id": "88",
            "user_id": "42",
            "message_id": "91",
            "likes": [{"emoji_id": "14", "count": 3}],
            "is_add": true
        })
    );
    Ok(())
}

fn event(
    message_type: u32,
    group: Option<(u32, &str)>,
    friend_name: Option<&str>,
) -> Result<AccountEvent, Box<dyn std::error::Error>> {
    event_with_rich(message_type, group, friend_name, None)
}

fn event_with_rich(
    message_type: u32,
    group: Option<(u32, &str)>,
    friend_name: Option<&str>,
    rich_text: Option<RichTextMessage>,
) -> Result<AccountEvent, Box<dyn std::error::Error>> {
    let reply_ids = vec![None; rich_text.as_ref().map_or(0, |rich| rich.elements().len())];
    event_with_rich_and_replies(message_type, group, friend_name, rich_text, reply_ids)
}

fn event_with_rich_and_replies(
    message_type: u32,
    group: Option<(u32, &str)>,
    friend_name: Option<&str>,
    rich_text: Option<RichTextMessage>,
    reply_ids: Vec<Option<u32>>,
) -> Result<AccountEvent, Box<dyn std::error::Error>> {
    let group = group.map(|(group_uin, member_name)| Route {
        group_uin,
        member_name: member_name.to_owned(),
        group_name: "group".to_owned(),
    });
    let body = PushBody {
        response: Some(Response {
            from_uin: 42,
            from_uid: Some("u_source".to_owned()),
            to_uin: 10_001,
            friend: friend_name.map(|name| Friend {
                name: Some(name.to_owned()),
            }),
            group,
        }),
        content: Some(Content {
            message_type,
            sequence: Some(7),
            timestamp: Some(1_800_000_000),
        }),
        body: Some(Body {
            content: Some(b"content".to_vec()),
        }),
    };
    let mut decoder = MessageDecoder::default();
    let MessageDisposition::New(envelope) = decoder.decode_embedded(&body.encode_to_vec())? else {
        return Err("expected new message".into());
    };
    let identity = identity()?;
    Ok(AccountEvent::Message(Box::new(
        InboundMessage::new(identity, 9, *envelope, rich_text).with_reply_ids(reply_ids)?,
    )))
}

fn identity() -> Result<AccountIdentity, account_api::EventHubError> {
    AccountIdentity::new(
        AccountLocalId::from_bytes([1; 16]),
        10_001,
        "self".to_owned(),
    )
}
