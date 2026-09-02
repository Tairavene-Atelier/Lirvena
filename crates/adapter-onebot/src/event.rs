use account_api::{
    AccountEvent, GroupRequestKind, InboundMessage, ResolvedFriendRequest, ResolvedGroupNotice,
    ResolvedGroupNoticeKind, ResolvedGroupRequest,
};
use account_runtime::AccountPhase;
use qq_message::{MemberDecreaseKind, MemberIncreaseKind, MentionTarget, MessageClass, Segment};
use serde_json::{Map, Value, json};

use crate::IdFormat;

/// Failure to project a validated account event without inventing semantics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EventProjectionError;

impl core::fmt::Display for EventProjectionError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("account event has no honest OneBot projection")
    }
}

impl std::error::Error for EventProjectionError {}

/// Projects an account event to canonical `OneBot` JSON.
///
/// Internal accounting-only events deliberately return `None`.
///
/// # Errors
///
/// Returns an error when an authenticated QQ event lacks the identity required by `OneBot`.
pub fn project_account_event(
    event: &AccountEvent,
    id_format: IdFormat,
) -> Result<Option<Value>, EventProjectionError> {
    match event {
        AccountEvent::IdentityReady(identity) => Ok(Some(json!({
            "time": 0,
            "self_id": id_format.value(identity.qq_id()),
            "post_type": "meta_event",
            "meta_event_type": "lifecycle",
            "sub_type": "connect"
        }))),
        AccountEvent::Lifecycle {
            phase,
            occurred_at_ms,
            ..
        } => Ok(Some(json!({
            "time": occurred_at_ms / 1000,
            "post_type": "meta_event",
            "meta_event_type": "lifecycle",
            "sub_type": lifecycle_subtype(*phase)
        }))),
        AccountEvent::Message(message) => project_message(message, id_format).map(Some),
        AccountEvent::GroupNotice(notice) => project_group_notice(notice, id_format).map(Some),
        AccountEvent::GroupRequest(request) => Ok(Some(project_group_request(request, id_format))),
        AccountEvent::FriendRequest(request) => {
            Ok(Some(project_friend_request(request, id_format)))
        }
        AccountEvent::OutboundMessageAccepted { .. } | AccountEvent::GroupCountObserved { .. } => {
            Ok(None)
        }
    }
}

fn project_friend_request(request: &ResolvedFriendRequest, id_format: IdFormat) -> Value {
    json!({
        "time": request.occurred_at(),
        "self_id": id_format.value(request.account().qq_id()),
        "post_type": "request",
        "request_type": "friend",
        "user_id": id_format.value(request.user_id()),
        "comment": request.comment(),
        "flag": request.reference().flag(),
    })
}

fn project_group_request(request: &ResolvedGroupRequest, id_format: IdFormat) -> Value {
    let sub_type = match request.kind() {
        GroupRequestKind::Join | GroupRequestKind::Invitation => "add",
        GroupRequestKind::SelfInvitation => "invite",
    };
    let mut object = Map::from_iter([
        ("time".to_owned(), json!(request.occurred_at())),
        (
            "self_id".to_owned(),
            id_format.value(request.account().qq_id()),
        ),
        ("post_type".to_owned(), json!("request")),
        ("request_type".to_owned(), json!("group")),
        ("sub_type".to_owned(), json!(sub_type)),
        (
            "group_id".to_owned(),
            id_format.value(u64::from(request.reference().group_id())),
        ),
        ("user_id".to_owned(), id_format.value(request.user_id())),
        ("comment".to_owned(), json!(request.comment())),
        ("flag".to_owned(), json!(request.reference().flag())),
    ]);
    if let Some(inviter_id) = request.inviter_id() {
        object.insert("invitor_id".to_owned(), id_format.value(inviter_id));
    }
    Value::Object(object)
}

fn project_group_notice(
    notice: &ResolvedGroupNotice,
    id_format: IdFormat,
) -> Result<Value, EventProjectionError> {
    let (notice_type, sub_type) = match notice.kind() {
        ResolvedGroupNoticeKind::AdministratorSet => ("group_admin", "set"),
        ResolvedGroupNoticeKind::AdministratorUnset => ("group_admin", "unset"),
        ResolvedGroupNoticeKind::MemberIncrease(MemberIncreaseKind::Approve) => {
            ("group_increase", "approve")
        }
        ResolvedGroupNoticeKind::MemberIncrease(MemberIncreaseKind::Invite) => {
            ("group_increase", "invite")
        }
        ResolvedGroupNoticeKind::MemberDecrease(MemberDecreaseKind::KickMe) => {
            ("group_decrease", "kick_me")
        }
        ResolvedGroupNoticeKind::MemberDecrease(MemberDecreaseKind::Disband) => {
            ("group_decrease", "disband")
        }
        ResolvedGroupNoticeKind::MemberDecrease(MemberDecreaseKind::Leave) => {
            ("group_decrease", "leave")
        }
        ResolvedGroupNoticeKind::MemberDecrease(MemberDecreaseKind::Kick) => {
            ("group_decrease", "kick")
        }
        ResolvedGroupNoticeKind::MemberIncrease(MemberIncreaseKind::Unknown(_))
        | ResolvedGroupNoticeKind::MemberDecrease(MemberDecreaseKind::Unknown(_)) => {
            return Err(EventProjectionError);
        }
    };
    let mut object = Map::from_iter([
        ("time".to_owned(), json!(notice.occurred_at())),
        (
            "self_id".to_owned(),
            id_format.value(notice.account().qq_id()),
        ),
        ("post_type".to_owned(), json!("notice")),
        ("notice_type".to_owned(), json!(notice_type)),
        ("sub_type".to_owned(), json!(sub_type)),
        ("group_id".to_owned(), id_format.value(notice.group_id())),
        ("user_id".to_owned(), id_format.value(notice.user_id())),
    ]);
    if let Some(operator_id) = notice.operator_id() {
        object.insert("operator_id".to_owned(), id_format.value(operator_id));
    }
    Ok(Value::Object(object))
}

fn project_message(
    message: &InboundMessage,
    id_format: IdFormat,
) -> Result<Value, EventProjectionError> {
    let envelope = message.envelope();
    let route = envelope.route();
    let (message_type, sub_type, user_id) = match envelope.class() {
        MessageClass::Private | MessageClass::PrivateRecord | MessageClass::PrivateFile => {
            ("private", "friend", u64::from(route.from_uin))
        }
        MessageClass::Group => ("group", "normal", u64::from(route.from_uin)),
        MessageClass::Temporary if route.group_uin.is_some() => {
            ("private", "group", u64::from(route.from_uin))
        }
        _ => return Err(EventProjectionError),
    };
    if user_id == 0 {
        return Err(EventProjectionError);
    }
    let segments = message
        .rich_text()
        .map(|rich| rich.elements().iter().map(segment_json).collect())
        .unwrap_or_default();
    let raw_message = message
        .rich_text()
        .map(|rich| rich.elements().iter().map(raw_segment).collect::<String>())
        .unwrap_or_default();
    let mut object = Map::from_iter([
        ("time".to_owned(), json!(envelope.timestamp().max(0))),
        (
            "self_id".to_owned(),
            id_format.value(message.account().qq_id()),
        ),
        ("post_type".to_owned(), json!("message")),
        ("message_type".to_owned(), json!(message_type)),
        ("sub_type".to_owned(), json!(sub_type)),
        (
            "message_id".to_owned(),
            id_format.value(envelope.sequence()),
        ),
        ("user_id".to_owned(), id_format.value(user_id)),
        ("message".to_owned(), Value::Array(segments)),
        ("raw_message".to_owned(), json!(raw_message)),
        ("font".to_owned(), json!(0)),
        ("sender".to_owned(), sender_json(route, user_id, id_format)),
    ]);
    if let Some(group_id) = route.group_uin {
        object.insert("group_id".to_owned(), id_format.value(u64::from(group_id)));
    }
    Ok(Value::Object(object))
}

fn sender_json(route: &qq_message::MessageRoute, user_id: u64, id_format: IdFormat) -> Value {
    let mut sender = Map::from_iter([("user_id".to_owned(), id_format.value(user_id))]);
    if let Some(name) = route
        .member_name
        .as_deref()
        .or(route.friend_name.as_deref())
    {
        sender.insert("nickname".to_owned(), json!(name));
    }
    if let Some(name) = route.member_name.as_deref() {
        sender.insert("card".to_owned(), json!(name));
    }
    Value::Object(sender)
}

fn segment_json(element: &qq_message::RichTextElement) -> Value {
    match element.segment() {
        Segment::Text(text) => json!({"type": "text", "data": {"text": text}}),
        Segment::Mention(mention) => {
            let qq = match mention.target() {
                MentionTarget::Everyone => "all".to_owned(),
                MentionTarget::Account(value) => value.to_string(),
                MentionTarget::User(value) => value.clone(),
                MentionTarget::Unresolved => mention.display().to_owned(),
            };
            json!({"type": "at", "data": {"qq": qq}})
        }
        Segment::Face(face) => json!({"type": "face", "data": {"id": face.id()}}),
        Segment::Image(image) => media_segment("image", image.file()),
        Segment::Video(video) => media_segment("video", video.file()),
        Segment::Voice(voice) => media_segment("record", voice.file()),
        Segment::Unsupported => json!({
            "type": "lirvena_unsupported",
            "data": {"encoded_size": element.encoded().len()}
        }),
    }
}

fn media_segment(kind: &str, file: &qq_message::MediaFile) -> Value {
    json!({
        "type": kind,
        "data": {
            "file": file.uuid().unwrap_or_else(|| file.name()),
            "url": file.remote_reference()
        }
    })
}

fn raw_segment(element: &qq_message::RichTextElement) -> String {
    match element.segment() {
        Segment::Text(text) => text.clone(),
        Segment::Mention(mention) => format!("@{}", mention.display()),
        Segment::Face(face) => format!("[CQ:face,id={}]", face.id()),
        Segment::Image(_) => "[CQ:image]".to_owned(),
        Segment::Video(_) => "[CQ:video]".to_owned(),
        Segment::Voice(_) => "[CQ:record]".to_owned(),
        Segment::Unsupported => "[CQ:lirvena_unsupported]".to_owned(),
    }
}

const fn lifecycle_subtype(phase: AccountPhase) -> &'static str {
    match phase {
        AccountPhase::Active => "enable",
        AccountPhase::Stopped | AccountPhase::ProtectiveOffline => "disable",
        AccountPhase::Starting => "connect",
    }
}
