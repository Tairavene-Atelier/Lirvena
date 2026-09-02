use std::collections::BTreeMap;

use account_api::{AccountActionError, AccountActionRequest, AccountIdentity};
use qq_directory::{
    FriendEntry, encode_friend_page_request, encode_group_list_request, parse_friend_page,
    parse_group_list,
};
use qq_message::{SendTextInput, SendTextTarget, encode_text_message, parse_send_message_response};
use serde_json::{Value, json};

use super::packets::{PacketContext, PacketRuntime};
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
        "can_send_image" | "can_send_record" => Ok(json!({"yes": true})),
        "get_friend_list" => {
            refresh_friends(packets, pushes, friends, context).await?;
            Ok(Value::Array(
                friends
                    .values()
                    .map(|friend| {
                        json!({
                            "user_id": friend.uin,
                            "nickname": friend.nickname,
                            "remark": friend.remark,
                        })
                    })
                    .collect(),
            ))
        }
        "get_group_list" => get_group_list(packets, pushes, context).await,
        "send_msg" => send_message(request, packets, pushes, friends, context).await,
        "send_group_msg" => send_group_text(request, packets, pushes, context).await,
        "send_private_msg" => send_private_text(request, packets, pushes, friends, context).await,
        _ => Err(AccountActionError::ActionNotFound),
    }
}

async fn get_group_list(
    packets: &PacketRuntime,
    pushes: &PushRuntime,
    context: &mut OnlineContext<'_>,
) -> Result<Value, AccountActionError> {
    let body = encode_group_list_request();
    let response = packets
        .send_with_reserve(
            packet_context(context, pushes),
            "OidbSvcTrpcTcp.0xfe5_2",
            &[],
            &body,
        )
        .await
        .map_err(|_error| AccountActionError::QqFailure)?;
    let groups = parse_group_list(&response).map_err(|_error| AccountActionError::QqFailure)?;
    Ok(Value::Array(
        groups
            .into_iter()
            .map(|group| {
                json!({
                    "group_id": group.group_id,
                    "group_name": group.group_name,
                    "member_count": group.member_count,
                    "max_member_count": group.max_member_count,
                })
            })
            .collect(),
    ))
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

async fn refresh_friends(
    packets: &PacketRuntime,
    pushes: &PushRuntime,
    friends: &mut BTreeMap<u32, FriendEntry>,
    context: &mut OnlineContext<'_>,
) -> Result<(), AccountActionError> {
    const MAX_PAGES: usize = 64;
    let mut collected = BTreeMap::new();
    let mut next = None;
    for _page in 0..MAX_PAGES {
        let body = encode_friend_page_request(next);
        let response = packets
            .send_with_reserve(
                packet_context(context, pushes),
                "OidbSvcTrpcTcp.0xfd4_1",
                &[],
                &body,
            )
            .await
            .map_err(|_error| AccountActionError::QqFailure)?;
        let page = parse_friend_page(&response).map_err(|_error| AccountActionError::QqFailure)?;
        for friend in page.friends {
            if collected.insert(friend.uin, friend).is_some() {
                return Err(AccountActionError::QqFailure);
            }
        }
        match page.next_uin {
            Some(value) if Some(value) != next => next = Some(value),
            Some(_) => return Err(AccountActionError::QqFailure),
            None => {
                *friends = collected;
                return Ok(());
            }
        }
    }
    Err(AccountActionError::QqFailure)
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
        refresh_friends(packets, pushes, friends, context).await?;
    }
    let uid = friends
        .get(&uin)
        .map(|friend| friend.uid.clone())
        .ok_or(AccountActionError::Unsupported)?;
    let text = plain_text(request.params().get("message"), request.params())?;
    send_text(
        SendTextTarget::Private { uin, uid: &uid },
        &text,
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
    let text = plain_text(request.params().get("message"), request.params())?;
    send_text(
        SendTextTarget::Group { group_code },
        &text,
        packets,
        pushes,
        context,
    )
    .await
}

async fn send_text(
    target: SendTextTarget<'_>,
    text: &str,
    packets: &PacketRuntime,
    pushes: &PushRuntime,
    context: &mut OnlineContext<'_>,
) -> Result<Value, AccountActionError> {
    let body = encode_text_message(&SendTextInput {
        target,
        text,
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
            packet_context(context, pushes),
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

fn packet_context<'a>(
    context: &'a mut OnlineContext<'_>,
    pushes: &'a PushRuntime,
) -> PacketContext<'a> {
    PacketContext {
        qq: context.qq,
        push_plan: pushes.plan(),
        profile: context.profile,
        credential: context.credential,
        uin: context.uin,
    }
}

fn required_u32(value: Option<&Value>) -> Result<u32, AccountActionError> {
    value
        .and_then(|value| match value {
            Value::Number(number) => number.as_u64(),
            Value::String(value) => value.parse().ok(),
            _ => None,
        })
        .and_then(|value| u32::try_from(value).ok())
        .filter(|value| *value != 0)
        .ok_or(AccountActionError::BadParameters)
}

fn plain_text(
    value: Option<&Value>,
    params: &serde_json::Map<String, Value>,
) -> Result<String, AccountActionError> {
    match value {
        Some(Value::String(value)) => {
            if params.get("auto_escape").and_then(Value::as_bool) != Some(true)
                && value.contains("[CQ:")
            {
                return Err(AccountActionError::Unsupported);
            }
            Ok(value.clone())
        }
        Some(Value::Array(segments)) => segments
            .iter()
            .map(|segment| {
                let object = segment
                    .as_object()
                    .ok_or(AccountActionError::BadParameters)?;
                if object.get("type").and_then(Value::as_str) != Some("text") {
                    return Err(AccountActionError::Unsupported);
                }
                object
                    .get("data")
                    .and_then(Value::as_object)
                    .and_then(|data| data.get("text"))
                    .and_then(Value::as_str)
                    .map(str::to_owned)
                    .ok_or(AccountActionError::BadParameters)
            })
            .collect::<Result<String, _>>(),
        Some(_) | None => Err(AccountActionError::BadParameters),
    }
}
