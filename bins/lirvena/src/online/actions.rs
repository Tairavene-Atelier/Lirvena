use std::collections::BTreeMap;

use account_api::{AccountActionError, AccountActionRequest, AccountIdentity};
use account_message_store::{QuoteTarget, RecallTarget};
use adapter_onebot::{MessageSegment, parse_message};
use qq_directory::FriendEntry;
use qq_message::{
    OutboundSegment, SendMessageInput, SendTextTarget, encode_message, parse_send_message_response,
};
use serde_json::{Value, json};

use super::controls;
use super::directory;
use super::essence;
use super::media::MediaRuntime;
use super::message_recall::recall_message;
use super::message_registry::{MessageRegistry, OutboundCorrelations};
use super::packets::{PacketContext, PacketRuntime};
use super::parameters::required_u32;
use super::push::PushRuntime;
use super::read_report::mark_message_read;
use super::runtime::OnlineContext;
use super::user_profile::stranger_info;
use crate::opaque::{OpaqueOperation, request_reserve};
use crate::support::{now_ms, now_seconds, random_nonzero_u32};

pub(super) struct ActionResources<'a> {
    pub(super) messages: &'a mut MessageRegistry,
    pub(super) media: &'a mut MediaRuntime,
}

pub(super) async fn execute_account_action(
    request: &AccountActionRequest,
    identity: &AccountIdentity,
    packets: &PacketRuntime,
    pushes: &PushRuntime,
    friends: &mut BTreeMap<u32, FriendEntry>,
    resources: &mut ActionResources<'_>,
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
        "can_send_image" | "can_send_record" => Ok(json!({"yes": true})),
        "get_friend_list" => directory::friend_list(packets, pushes, friends, context).await,
        "get_stranger_info" => {
            stranger_info(
                request.params().get("user_id"),
                request.params().get("no_cache"),
                packets,
                pushes,
                context,
            )
            .await
        }
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
        "get_group_requests" => {
            super::requests::list_group_requests(identity, packets, pushes, context).await
        }
        "get_friend_requests" => {
            super::requests::list_friend_requests(identity, packets, pushes, context).await
        }
        "get_msg" => get_message(request, resources.messages),
        "get_forward_msg" => {
            super::long_message::get_forward_message(request, packets, pushes, context).await
        }
        "get_group_msg_history" => {
            super::history::group(
                request,
                identity,
                packets,
                pushes,
                resources.messages,
                context,
            )
            .await
        }
        "get_friend_msg_history" => {
            super::history::friend(
                request,
                identity,
                packets,
                pushes,
                friends,
                resources.messages,
                context,
            )
            .await
        }
        "send_group_forward_msg" => {
            super::long_message::send_group_forward_message(
                request, identity, packets, pushes, resources, context,
            )
            .await
        }
        "send_forward_msg" => {
            super::long_message::upload_forward_message(
                request, identity, packets, pushes, resources, context,
            )
            .await
        }
        "send_private_forward_msg" => {
            super::long_message::send_private_forward_message(
                request, identity, packets, pushes, friends, resources, context,
            )
            .await
        }
        "send_msg" => {
            send_message(
                request, identity, packets, pushes, friends, resources, context,
            )
            .await
        }
        "send_group_msg" => {
            send_group_text(request, identity, packets, pushes, resources, context).await
        }
        "send_private_msg" => {
            send_private_text(
                request, identity, packets, pushes, friends, resources, context,
            )
            .await
        }
        "delete_msg" => recall_message(request, packets, pushes, resources.messages, context).await,
        "mark_msg_as_read" => {
            mark_message_read(request, packets, pushes, resources.messages, context).await
        }
        "set_essence_msg" => {
            essence::update(request, true, packets, pushes, resources.messages, context).await
        }
        "delete_essence_msg" => {
            essence::update(request, false, packets, pushes, resources.messages, context).await
        }
        "set_group_reaction"
        | "set_msg_emoji_like"
        | ".join_group_emoji_chain"
        | ".join_friend_emoji_chain" => {
            super::reaction::execute(request, packets, pushes, resources.messages, context).await
        }
        "send_poke"
        | "group_poke"
        | "friend_poke"
        | "send_like"
        | "set_group_kick"
        | "set_group_ban"
        | "set_group_whole_ban"
        | "set_group_admin"
        | "set_group_card"
        | "set_group_name"
        | "set_group_leave"
        | "set_group_special_title"
        | "set_group_add_request"
        | "set_friend_add_request" => {
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
    identity: &AccountIdentity,
    packets: &PacketRuntime,
    pushes: &PushRuntime,
    friends: &mut BTreeMap<u32, FriendEntry>,
    resources: &mut ActionResources<'_>,
    context: &mut OnlineContext<'_>,
) -> Result<Value, AccountActionError> {
    match request.params().get("message_type").and_then(Value::as_str) {
        Some("private") => {
            send_private_text(
                request, identity, packets, pushes, friends, resources, context,
            )
            .await
        }
        Some("group") => {
            send_group_text(request, identity, packets, pushes, resources, context).await
        }
        Some(_) => Err(AccountActionError::BadParameters),
        None => match (
            request.params().contains_key("user_id"),
            request.params().contains_key("group_id"),
        ) {
            (true, false) => {
                send_private_text(
                    request, identity, packets, pushes, friends, resources, context,
                )
                .await
            }
            (false, true) => {
                send_group_text(request, identity, packets, pushes, resources, context).await
            }
            _ => Err(AccountActionError::BadParameters),
        },
    }
}

async fn send_private_text(
    request: &AccountActionRequest,
    identity: &AccountIdentity,
    packets: &PacketRuntime,
    pushes: &PushRuntime,
    friends: &mut BTreeMap<u32, FriendEntry>,
    resources: &mut ActionResources<'_>,
    context: &mut OnlineContext<'_>,
) -> Result<Value, AccountActionError> {
    let uin = required_u32(request.params().get("user_id"))?;
    let uid = resolve_private_uid(uin, packets, pushes, friends, context).await?;
    let target = SendTextTarget::Private { uin, uid: &uid };
    let parsed = parse_segments(request)?;
    let replies = resolve_reply_segments(&parsed, &target, resources.messages)?;
    let segments = compile_segments(
        &parsed,
        &target,
        &replies,
        packets,
        pushes,
        resources.media,
        context,
    )
    .await?;
    send_segments(
        target,
        &segments,
        identity,
        packets,
        pushes,
        resources.messages,
        context,
    )
    .await
}

async fn send_group_text(
    request: &AccountActionRequest,
    identity: &AccountIdentity,
    packets: &PacketRuntime,
    pushes: &PushRuntime,
    resources: &mut ActionResources<'_>,
    context: &mut OnlineContext<'_>,
) -> Result<Value, AccountActionError> {
    if request.params().get("at_sender").and_then(Value::as_bool) == Some(true) {
        return Err(AccountActionError::Unsupported);
    }
    let group_code = required_u32(request.params().get("group_id"))?;
    let target = SendTextTarget::Group { group_code };
    let parsed = parse_segments(request)?;
    let replies = resolve_reply_segments(&parsed, &target, resources.messages)?;
    let segments = compile_segments(
        &parsed,
        &target,
        &replies,
        packets,
        pushes,
        resources.media,
        context,
    )
    .await?;
    send_segments(
        target,
        &segments,
        identity,
        packets,
        pushes,
        resources.messages,
        context,
    )
    .await
}

pub(super) async fn send_segments(
    target: SendTextTarget<'_>,
    segments: &[CompiledSegment],
    identity: &AccountIdentity,
    packets: &PacketRuntime,
    pushes: &PushRuntime,
    messages: &mut MessageRegistry,
    context: &mut OnlineContext<'_>,
) -> Result<Value, AccountActionError> {
    let outbound = segments
        .iter()
        .map(CompiledSegment::borrowed)
        .collect::<Vec<_>>();
    let client_sequence = random_nonzero_u32().map_err(|_error| AccountActionError::QqFailure)?;
    let random = random_nonzero_u32().map_err(|_error| AccountActionError::QqFailure)?;
    let unix_seconds = now_seconds().map_err(|_error| AccountActionError::QqFailure)?;
    let body = encode_message(&SendMessageInput {
        target: target.clone(),
        segments: &outbound,
        client_sequence,
        random,
        unix_seconds,
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
    let timestamp = if outcome.timestamp == 0 {
        unix_seconds
    } else {
        outcome.timestamp
    };
    let record = outbound_message_record(identity, &target, segments, timestamp);
    let message_id = messages
        .register_outbound(
            &target,
            OutboundCorrelations {
                sequence: outcome.sequence,
                client_sequence,
                random,
                timestamp,
            },
            now_ms().map_err(|_error| AccountActionError::QqFailure)?,
            record,
        )
        .map_err(|_error| AccountActionError::QqFailure)?;
    Ok(json!({"message_id": message_id}))
}

fn get_message(
    request: &AccountActionRequest,
    messages: &MessageRegistry,
) -> Result<Value, AccountActionError> {
    let message_id = required_u32(request.params().get("message_id"))?;
    messages
        .get(message_id)
        .map_err(|_error| AccountActionError::QqFailure)?
        .map(|record| record.response().clone())
        .ok_or(AccountActionError::QqFailure)
}

fn outbound_message_record(
    identity: &AccountIdentity,
    target: &SendTextTarget<'_>,
    segments: &[CompiledSegment],
    timestamp: u32,
) -> Value {
    let message_type = match target {
        SendTextTarget::Group { .. } => "group",
        SendTextTarget::Private { .. } => "private",
    };
    json!({
        "time": timestamp,
        "message_type": message_type,
        "sender": {
            "user_id": identity.qq_id(),
            "nickname": identity.nickname(),
        },
        "message": segments.iter().map(CompiledSegment::onebot_value).collect::<Vec<_>>(),
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum CompiledSegment {
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
    Image {
        source: String,
        group: bool,
        message_info: Vec<u8>,
        compatibility: Vec<u8>,
    },
    Record {
        source: String,
        group: bool,
        message_info: Vec<u8>,
    },
    Video {
        source: String,
        group: bool,
        message_info: Vec<u8>,
        compatibility: Vec<u8>,
    },
    Json(String),
    Forward {
        resource_id: String,
        card: String,
    },
    Xml {
        body: String,
        service_id: i32,
    },
    Poke {
        kind: u32,
        strength: u32,
    },
    Reply {
        message_id: u32,
        group: bool,
        quote: QuoteTarget,
    },
}

impl CompiledSegment {
    pub(super) fn borrowed(&self) -> OutboundSegment<'_> {
        match self {
            Self::Text(value) => OutboundSegment::Text(value),
            Self::MentionEveryone { display } => OutboundSegment::MentionEveryone { display },
            Self::Mention { uin, uid, display } => OutboundSegment::Mention {
                uin: *uin,
                uid,
                display,
            },
            Self::Face(value) => OutboundSegment::Face(*value),
            Self::Image {
                group,
                message_info,
                compatibility,
                ..
            } => OutboundSegment::Image {
                group: *group,
                message_info,
                compatibility,
            },
            Self::Record {
                group,
                message_info,
                ..
            } => OutboundSegment::Record {
                group: *group,
                message_info,
            },
            Self::Video {
                group,
                message_info,
                compatibility,
                ..
            } => OutboundSegment::Video {
                group: *group,
                message_info,
                compatibility,
            },
            Self::Json(body) => OutboundSegment::Json(body),
            Self::Forward { card, .. } => OutboundSegment::Json(card),
            Self::Xml { body, service_id } => OutboundSegment::Xml {
                body,
                service_id: *service_id,
            },
            Self::Poke { kind, strength } => OutboundSegment::Poke {
                kind: *kind,
                strength: *strength,
            },
            Self::Reply { group, quote, .. } => OutboundSegment::Reply {
                group: *group,
                sequence: quote.sequence(),
                message_uid: quote.message_uid(),
                sender_uin: quote.sender_uin(),
                sender_uid: quote.sender_uid(),
                timestamp: quote.timestamp(),
                elements: quote.elements(),
            },
        }
    }

    pub(super) fn preview_text(&self) -> &str {
        match self {
            Self::Text(value) => value,
            Self::MentionEveryone { display } | Self::Mention { display, .. } => display,
            Self::Face(_) => "[表情]",
            Self::Image { .. } => "[图片]",
            Self::Record { .. } => "[语音]",
            Self::Video { .. } => "[视频]",
            Self::Json(_) => "[JSON消息]",
            Self::Forward { .. } => "[聊天记录]",
            Self::Xml { .. } => "[XML消息]",
            Self::Poke { .. } => "[戳一戳]",
            Self::Reply { .. } => "[回复]",
        }
    }

    fn onebot_value(&self) -> Value {
        match self {
            Self::Text(value) => json!({"type": "text", "data": {"text": value}}),
            Self::MentionEveryone { .. } => json!({"type": "at", "data": {"qq": "all"}}),
            Self::Mention { uin, .. } => json!({"type": "at", "data": {"qq": uin}}),
            Self::Face(value) => json!({"type": "face", "data": {"id": value}}),
            Self::Image { source, .. } => json!({"type": "image", "data": {"file": source}}),
            Self::Record { source, .. } => json!({"type": "record", "data": {"file": source}}),
            Self::Video { source, .. } => json!({"type": "video", "data": {"file": source}}),
            Self::Json(body) => json!({"type": "json", "data": {"data": body}}),
            Self::Forward { resource_id, .. } => {
                json!({"type": "forward", "data": {"id": resource_id}})
            }
            Self::Xml { body, service_id } => {
                json!({"type": "xml", "data": {"data": body, "service_id": service_id}})
            }
            Self::Poke { kind, strength } => {
                json!({"type": "poke", "data": {"type": kind, "strength": strength, "id": -1}})
            }
            Self::Reply { message_id, .. } => {
                json!({"type": "reply", "data": {"id": message_id}})
            }
        }
    }
}

fn parse_segments(
    request: &AccountActionRequest,
) -> Result<Vec<MessageSegment>, AccountActionError> {
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
    parse_message(raw, auto_escape).map_err(|_error| AccountActionError::BadParameters)
}

fn resolve_reply_segments(
    segments: &[MessageSegment],
    target: &SendTextTarget<'_>,
    messages: &MessageRegistry,
) -> Result<BTreeMap<u32, (bool, QuoteTarget)>, AccountActionError> {
    let mut replies = BTreeMap::new();
    for segment in segments.iter().filter(|segment| segment.kind() == "reply") {
        let message_id =
            segment_u32(segment.data().get("id")).ok_or(AccountActionError::BadParameters)?;
        if replies.contains_key(&message_id) {
            continue;
        }
        let record = messages
            .get(message_id)
            .map_err(|_error| AccountActionError::QqFailure)?
            .ok_or(AccountActionError::QqFailure)?;
        let quote = record.quote().ok_or(AccountActionError::QqFailure)?;
        let group = quote_scope_matches(record.recall(), target)?;
        replies.insert(message_id, (group, quote.clone()));
    }
    Ok(replies)
}

pub(super) async fn compile_segments(
    segments: &[MessageSegment],
    target: &SendTextTarget<'_>,
    replies: &BTreeMap<u32, (bool, QuoteTarget)>,
    packets: &PacketRuntime,
    pushes: &PushRuntime,
    media: &mut MediaRuntime,
    context: &mut OnlineContext<'_>,
) -> Result<Vec<CompiledSegment>, AccountActionError> {
    let needs_members = segments.iter().any(|segment| {
        segment.kind() == "at" && segment.data().get("qq").and_then(Value::as_str) != Some("all")
    });
    let group_id = match target {
        SendTextTarget::Group { group_code } => Some(*group_code),
        SendTextTarget::Private { .. } => None,
    };
    let members = match (group_id, needs_members) {
        (Some(group_id), true) => {
            Some(directory::group_members(group_id, packets, pushes, context).await?)
        }
        (None, true) => return Err(AccountActionError::Unsupported),
        (_, false) => None,
    };
    let lookups = SegmentLookups {
        members: members.as_deref(),
        replies,
    };
    let mut compiled = Vec::with_capacity(segments.len());
    for segment in segments {
        compiled.push(
            compile_segment(segment, target, &lookups, packets, pushes, media, context).await?,
        );
    }
    Ok(compiled)
}

pub(super) async fn resolve_private_uid(
    uin: u32,
    packets: &PacketRuntime,
    pushes: &PushRuntime,
    friends: &mut BTreeMap<u32, FriendEntry>,
    context: &mut OnlineContext<'_>,
) -> Result<String, AccountActionError> {
    if !friends.contains_key(&uin) {
        directory::refresh_friends(packets, pushes, friends, context).await?;
    }
    friends
        .get(&uin)
        .map(|friend| friend.uid.clone())
        .ok_or(AccountActionError::Unsupported)
}

struct SegmentLookups<'a> {
    members: Option<&'a [qq_directory::GroupMember]>,
    replies: &'a BTreeMap<u32, (bool, QuoteTarget)>,
}

async fn compile_segment(
    segment: &MessageSegment,
    target: &SendTextTarget<'_>,
    lookups: &SegmentLookups<'_>,
    packets: &PacketRuntime,
    pushes: &PushRuntime,
    media: &mut MediaRuntime,
    context: &mut OnlineContext<'_>,
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
        "at" if matches!(target, SendTextTarget::Group { .. }) => {
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
            let member = lookups
                .members
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
        "image" => {
            let source = segment
                .data()
                .get("file")
                .and_then(Value::as_str)
                .ok_or(AccountActionError::BadParameters)?;
            let image_target = match target {
                SendTextTarget::Private { uid, .. } => qq_media::MediaTarget::Direct(uid),
                SendTextTarget::Group { group_code } => qq_media::MediaTarget::Group(*group_code),
            };
            let uploaded = media
                .upload_image(source, image_target, packets, pushes, context)
                .await?;
            Ok(CompiledSegment::Image {
                source: source.to_owned(),
                group: uploaded.group,
                message_info: uploaded.message_info,
                compatibility: uploaded.compatibility,
            })
        }
        "record" => {
            let source = segment
                .data()
                .get("file")
                .and_then(Value::as_str)
                .ok_or(AccountActionError::BadParameters)?;
            let record_target = match target {
                SendTextTarget::Private { uid, .. } => qq_media::MediaTarget::Direct(uid),
                SendTextTarget::Group { group_code } => qq_media::MediaTarget::Group(*group_code),
            };
            let uploaded = media
                .upload_record(source, record_target, packets, pushes, context)
                .await?;
            Ok(CompiledSegment::Record {
                source: source.to_owned(),
                group: uploaded.group,
                message_info: uploaded.message_info,
            })
        }
        "video" => {
            let source = segment
                .data()
                .get("file")
                .and_then(Value::as_str)
                .ok_or(AccountActionError::BadParameters)?;
            let video_target = match target {
                SendTextTarget::Private { uid, .. } => qq_media::MediaTarget::Direct(uid),
                SendTextTarget::Group { group_code } => qq_media::MediaTarget::Group(*group_code),
            };
            let uploaded = media
                .upload_video(source, video_target, packets, pushes, context)
                .await?;
            Ok(CompiledSegment::Video {
                source: source.to_owned(),
                group: uploaded.group,
                message_info: uploaded.message_info,
                compatibility: uploaded.compatibility,
            })
        }
        "json" => segment
            .data()
            .get("data")
            .and_then(Value::as_str)
            .map(|body| CompiledSegment::Json(body.to_owned()))
            .ok_or(AccountActionError::BadParameters),
        "xml" => {
            let body = segment
                .data()
                .get("data")
                .and_then(Value::as_str)
                .ok_or(AccountActionError::BadParameters)?;
            let service_id = match segment.data().get("service_id") {
                Some(value) => segment_u32(Some(value)).ok_or(AccountActionError::BadParameters)?,
                None => 35,
            };
            Ok(CompiledSegment::Xml {
                body: body.to_owned(),
                service_id: i32::try_from(service_id)
                    .map_err(|_error| AccountActionError::BadParameters)?,
            })
        }
        "poke" => {
            let kind =
                segment_u32(segment.data().get("type")).ok_or(AccountActionError::BadParameters)?;
            let strength = match segment.data().get("strength") {
                Some(value) => segment_u32(Some(value)).ok_or(AccountActionError::BadParameters)?,
                None => 0,
            };
            Ok(CompiledSegment::Poke { kind, strength })
        }
        "reply" => {
            let message_id =
                segment_u32(segment.data().get("id")).ok_or(AccountActionError::BadParameters)?;
            let (group, quote) = lookups
                .replies
                .get(&message_id)
                .ok_or(AccountActionError::QqFailure)?;
            Ok(CompiledSegment::Reply {
                message_id,
                group: *group,
                quote: quote.clone(),
            })
        }
        _ => Err(AccountActionError::Unsupported),
    }
}

fn quote_scope_matches(
    recall: &RecallTarget,
    target: &SendTextTarget<'_>,
) -> Result<bool, AccountActionError> {
    match (recall, target) {
        (
            RecallTarget::Group { group_code, .. },
            SendTextTarget::Group {
                group_code: destination,
            },
        ) if group_code == destination => Ok(true),
        (
            RecallTarget::Private { uid, .. },
            SendTextTarget::Private {
                uid: destination, ..
            },
        ) if uid == destination => Ok(false),
        _ => Err(AccountActionError::BadParameters),
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

#[cfg(test)]
mod tests {
    use account_message_store::RecallTarget;
    use qq_message::{OutboundSegment, SendTextTarget};
    use serde_json::json;

    use super::{CompiledSegment, quote_scope_matches};

    #[test]
    fn compiled_forward_preserves_onebot_semantics() {
        let segment = CompiledSegment::Forward {
            resource_id: "forward-resource".to_owned(),
            card: "{\"app\":\"com.tencent.multimsg\"}".to_owned(),
        };

        assert_eq!(
            segment.onebot_value(),
            json!({"type": "forward", "data": {"id": "forward-resource"}})
        );
        assert_eq!(
            segment.borrowed(),
            OutboundSegment::Json("{\"app\":\"com.tencent.multimsg\"}")
        );
    }

    #[test]
    fn reply_scope_cannot_cross_conversations() {
        let group = RecallTarget::Group {
            group_code: 42,
            sequence: 7,
            random: Some(9),
        };
        assert_eq!(
            quote_scope_matches(&group, &SendTextTarget::Group { group_code: 42 }),
            Ok(true)
        );
        assert!(quote_scope_matches(&group, &SendTextTarget::Group { group_code: 43 }).is_err());

        let private = RecallTarget::Private {
            uid: "u_peer".to_owned(),
            peer_uin: Some(11),
            sequence: 7,
            client_sequence: 8,
            random: 9,
            timestamp: 10,
        };
        assert_eq!(
            quote_scope_matches(
                &private,
                &SendTextTarget::Private {
                    uin: 11,
                    uid: "u_peer",
                },
            ),
            Ok(false)
        );
        assert!(
            quote_scope_matches(
                &private,
                &SendTextTarget::Private {
                    uin: 12,
                    uid: "u_other",
                },
            )
            .is_err()
        );
    }
}
