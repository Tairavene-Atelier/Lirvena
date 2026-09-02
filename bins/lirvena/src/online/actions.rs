use std::collections::BTreeMap;

use account_api::{AccountActionError, AccountActionRequest, AccountIdentity};
use adapter_onebot::{MessageSegment, parse_message};
use qq_directory::FriendEntry;
use qq_message::{
    OutboundSegment, SendMessageInput, SendTextTarget, encode_message, parse_send_message_response,
};
use serde_json::{Value, json};

use super::controls;
use super::directory;
use super::packets::{PacketContext, PacketRuntime};
use super::parameters::required_u32;
use super::push::PushRuntime;
use super::runtime::OnlineContext;
use crate::opaque::{OpaqueOperation, request_reserve};
use crate::support::{now_seconds, random_nonzero_u32};

pub(super) async fn execute_account_action(
    request: &AccountActionRequest,
    identity: &AccountIdentity,
    packets: &PacketRuntime,
    pushes: &PushRuntime,
    friends: &mut BTreeMap<u32, FriendEntry>,
    context: &mut OnlineContext<'_>,
) -> Result<Value, AccountActionError> {
    match request.action() {
        "get_login_info" => Ok(json!({
            "user_id": identity.qq_id(),
            "nickname": identity.nickname(),
        })),
        "get_status" => Ok(json!({"online": true, "good": true})),
        "get_version_info" => Ok(json!({
            "app_name": "Lirvena",
            "app_version": env!("CARGO_PKG_VERSION"),
            "protocol_version": "v11",
        })),
        "can_send_image" | "can_send_record" => Ok(json!({"yes": false})),
        "get_friend_list" => directory::friend_list(packets, pushes, friends, context).await,
        "get_group_list" => directory::group_list(packets, pushes, context).await,
        "get_group_info" => {
            directory::group_info(request.params().get("group_id"), packets, pushes, context).await
        }
        "get_group_member_list" => {
            directory::group_member_list(request.params().get("group_id"), packets, pushes, context)
                .await
        }
        "get_group_member_info" => {
            directory::group_member_info(
                request.params().get("group_id"),
                request.params().get("user_id"),
                packets,
                pushes,
                context,
            )
            .await
        }
        "send_msg" => send_message(request, packets, pushes, friends, context).await,
        "send_group_msg" => send_group_text(request, packets, pushes, context).await,
        "send_private_msg" => send_private_text(request, packets, pushes, friends, context).await,
        "send_like"
        | "set_group_kick"
        | "set_group_ban"
        | "set_group_whole_ban"
        | "set_group_admin"
        | "set_group_card"
        | "set_group_name"
        | "set_group_leave"
        | "set_group_special_title" => {
            controls::execute(request, packets, pushes, friends, context).await
        }
        "clean_cache" => {
            friends.clear();
            Ok(json!({}))
        }
        _ => Err(AccountActionError::ActionNotFound),
    }
}

async fn send_message(
    request: &AccountActionRequest,
    packets: &PacketRuntime,
    pushes: &PushRuntime,
    friends: &mut BTreeMap<u32, FriendEntry>,
    context: &mut OnlineContext<'_>,
) -> Result<Value, AccountActionError> {
    match request.params().get("message_type").and_then(Value::as_str) {
        Some("private") => send_private_text(request, packets, pushes, friends, context).await,
        Some("group") => send_group_text(request, packets, pushes, context).await,
        Some(_) => Err(AccountActionError::BadParameters),
        None => match (
            request.params().contains_key("user_id"),
            request.params().contains_key("group_id"),
        ) {
            (true, false) => send_private_text(request, packets, pushes, friends, context).await,
            (false, true) => send_group_text(request, packets, pushes, context).await,
            _ => Err(AccountActionError::BadParameters),
        },
    }
}

async fn send_private_text(
    request: &AccountActionRequest,
    packets: &PacketRuntime,
    pushes: &PushRuntime,
    friends: &mut BTreeMap<u32, FriendEntry>,
    context: &mut OnlineContext<'_>,
) -> Result<Value, AccountActionError> {
    let uin = required_u32(request.params().get("user_id"))?;
    if !friends.contains_key(&uin) {
        directory::refresh_friends(packets, pushes, friends, context).await?;
    }
    let uid = friends
        .get(&uin)
        .map(|friend| friend.uid.clone())
        .ok_or(AccountActionError::Unsupported)?;
    let segments = compile_segments(request, None, packets, pushes, context).await?;
    send_segments(
        SendTextTarget::Private { uin, uid: &uid },
        &segments,
        packets,
        pushes,
        context,
    )
    .await
}

async fn send_group_text(
    request: &AccountActionRequest,
    packets: &PacketRuntime,
    pushes: &PushRuntime,
    context: &mut OnlineContext<'_>,
) -> Result<Value, AccountActionError> {
    if request.params().get("at_sender").and_then(Value::as_bool) == Some(true) {
        return Err(AccountActionError::Unsupported);
    }
    let group_code = required_u32(request.params().get("group_id"))?;
    let segments = compile_segments(request, Some(group_code), packets, pushes, context).await?;
    send_segments(
        SendTextTarget::Group { group_code },
        &segments,
        packets,
        pushes,
        context,
    )
    .await
}

async fn send_segments(
    target: SendTextTarget<'_>,
    segments: &[CompiledSegment],
    packets: &PacketRuntime,
    pushes: &PushRuntime,
    context: &mut OnlineContext<'_>,
) -> Result<Value, AccountActionError> {
    let outbound = segments
        .iter()
        .map(CompiledSegment::borrowed)
        .collect::<Vec<_>>();
    let body = encode_message(&SendMessageInput {
        target,
        segments: &outbound,
        client_sequence: random_nonzero_u32().map_err(|_error| AccountActionError::QqFailure)?,
        random: random_nonzero_u32().map_err(|_error| AccountActionError::QqFailure)?,
        unix_seconds: now_seconds().map_err(|_error| AccountActionError::QqFailure)?,
    })
    .map_err(|_error| AccountActionError::BadParameters)?;
    let reserve = request_reserve(
        context.ceylith,
        context.account_slot_id,
        OpaqueOperation::C,
        &body,
    )
    .await
    .map_err(|_error| AccountActionError::QqFailure)?;
    let response = packets
        .send_with_reserve(
            PacketContext::for_account(context, pushes.plan()),
            "MessageSvc.PbSendMsg",
            &reserve,
            &body,
        )
        .await
        .map_err(|_error| AccountActionError::QqFailure)?;
    let outcome =
        parse_send_message_response(&response).map_err(|_error| AccountActionError::QqFailure)?;
    if outcome.result != 0 {
        return Err(AccountActionError::QqFailure);
    }
    Ok(json!({"message_id": outcome.sequence}))
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum CompiledSegment {
    Text(String),
    MentionEveryone {
        display: String,
    },
    Mention {
        uin: u32,
        uid: String,
        display: String,
    },
    Face(u16),
}

impl CompiledSegment {
    fn borrowed(&self) -> OutboundSegment<'_> {
        match self {
            Self::Text(value) => OutboundSegment::Text(value),
            Self::MentionEveryone { display } => OutboundSegment::MentionEveryone { display },
            Self::Mention { uin, uid, display } => OutboundSegment::Mention {
                uin: *uin,
                uid,
                display,
            },
            Self::Face(value) => OutboundSegment::Face(*value),
        }
    }
}

async fn compile_segments(
    request: &AccountActionRequest,
    group_id: Option<u32>,
    packets: &PacketRuntime,
    pushes: &PushRuntime,
    context: &mut OnlineContext<'_>,
) -> Result<Vec<CompiledSegment>, AccountActionError> {
    let auto_escape = request
        .params()
        .get("auto_escape")
        .map(|value| value.as_bool().ok_or(AccountActionError::BadParameters))
        .transpose()?
        .unwrap_or(false);
    let raw = request
        .params()
        .get("message")
        .ok_or(AccountActionError::BadParameters)?;
    let segments =
        parse_message(raw, auto_escape).map_err(|_error| AccountActionError::BadParameters)?;
    let needs_members = segments.iter().any(|segment| {
        segment.kind() == "at" && segment.data().get("qq").and_then(Value::as_str) != Some("all")
    });
    let members = match (group_id, needs_members) {
        (Some(group_id), true) => {
            Some(directory::group_members(group_id, packets, pushes, context).await?)
        }
        (None, true) => return Err(AccountActionError::Unsupported),
        (_, false) => None,
    };
    segments
        .iter()
        .map(|segment| compile_segment(segment, group_id, members.as_deref()))
        .collect()
}

fn compile_segment(
    segment: &MessageSegment,
    group_id: Option<u32>,
    members: Option<&[qq_directory::GroupMember]>,
) -> Result<CompiledSegment, AccountActionError> {
    match segment.kind() {
        "text" => segment
            .data()
            .get("text")
            .and_then(Value::as_str)
            .map(|value| CompiledSegment::Text(value.to_owned()))
            .ok_or(AccountActionError::BadParameters),
        "face" => segment_u32(segment.data().get("id"))
            .and_then(|value| u16::try_from(value).ok())
            .map(CompiledSegment::Face)
            .ok_or(AccountActionError::BadParameters),
        "at" if group_id.is_some() => {
            let target = segment
                .data()
                .get("qq")
                .ok_or(AccountActionError::BadParameters)?;
            if target.as_str() == Some("all") {
                return Ok(CompiledSegment::MentionEveryone {
                    display: "@全体成员".to_owned(),
                });
            }
            let uin = segment_u32(Some(target)).ok_or(AccountActionError::BadParameters)?;
            let member = members
                .and_then(|values| values.iter().find(|value| value.uin == uin))
                .ok_or(AccountActionError::QqFailure)?;
            let name = if member.card.is_empty() {
                &member.nickname
            } else {
                &member.card
            };
            Ok(CompiledSegment::Mention {
                uin,
                uid: member.uid.clone(),
                display: format!("@{name}"),
            })
        }
        _ => Err(AccountActionError::Unsupported),
    }
}

fn segment_u32(value: Option<&Value>) -> Option<u32> {
    value
        .and_then(|value| match value {
            Value::Number(number) => number.as_u64(),
            Value::String(value) => value.parse().ok(),
            _ => None,
        })
        .and_then(|value| u32::try_from(value).ok())
}
