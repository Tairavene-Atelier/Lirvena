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
        "get_msg" => get_message(request, resources.messages),
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
        "send_like"
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
    if !friends.contains_key(&uin) {
        directory::refresh_friends(packets, pushes, friends, context).await?;
    }
    let uid = friends
        .get(&uin)
        .map(|friend| friend.uid.clone())
        .ok_or(AccountActionError::Unsupported)?;
    let target = SendTextTarget::Private { uin, uid: &uid };
    let segments =
        compile_segments(request, &target, packets, pushes, resources.media, context).await?;
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
    let segments =
        compile_segments(request, &target, packets, pushes, resources.media, context).await?;
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

async fn send_segments(
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
    Xml {
        body: String,
        service_id: i32,
    },
    Poke {
        kind: u32,
        strength: u32,
    },
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
            Self::Xml { body, service_id } => OutboundSegment::Xml {
                body,
                service_id: *service_id,
            },
            Self::Poke { kind, strength } => OutboundSegment::Poke {
                kind: *kind,
                strength: *strength,
            },
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
            Self::Xml { body, service_id } => {
                json!({"type": "xml", "data": {"data": body, "service_id": service_id}})
            }
            Self::Poke { kind, strength } => {
                json!({"type": "poke", "data": {"type": kind, "strength": strength, "id": -1}})
            }
        }
    }
}

async fn compile_segments(
    request: &AccountActionRequest,
    target: &SendTextTarget<'_>,
    packets: &PacketRuntime,
    pushes: &PushRuntime,
    media: &mut MediaRuntime,
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
    let mut compiled = Vec::with_capacity(segments.len());
    for segment in &segments {
        compiled.push(
            compile_segment(
                segment,
                target,
                members.as_deref(),
                packets,
                pushes,
                media,
                context,
            )
            .await?,
        );
    }
    Ok(compiled)
}

async fn compile_segment(
    segment: &MessageSegment,
    target: &SendTextTarget<'_>,
    members: Option<&[qq_directory::GroupMember]>,
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
