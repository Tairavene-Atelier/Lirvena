use std::collections::BTreeMap;

use account_api::{AccountActionError, AccountActionRequest, AccountIdentity};
use qq_directory::FriendEntry;
use qq_message::{SendTextInput, SendTextTarget, encode_text_message, parse_send_message_response};
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
        "can_send_image" | "can_send_record" => Ok(json!({"yes": true})),
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
        "set_group_kick"
        | "set_group_ban"
        | "set_group_whole_ban"
        | "set_group_admin"
        | "set_group_card"
        | "set_group_name"
        | "set_group_leave"
        | "set_group_special_title" => controls::execute(request, packets, pushes, context).await,
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
