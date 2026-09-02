use std::collections::BTreeMap;

use account_api::{
    AccountActionError, AccountActionRequest, FriendRequestReference, GroupRequestReference,
};
use qq_control::{
    ControlRequest, friend_like, friend_request, group_admin, group_ban, group_card, group_kick,
    group_leave, group_name, group_request, group_special_title, group_whole_ban,
    parse_control_response,
};
use qq_directory::FriendEntry;
use serde_json::{Value, json};

use super::directory;
use super::packets::{PacketContext, PacketRuntime};
use super::parameters::{optional_bool, optional_u32, required_text, required_u32};
use super::push::PushRuntime;
use super::runtime::OnlineContext;
use crate::opaque::{OpaqueOperation, request_reserve};

pub(super) async fn execute(
    request: &AccountActionRequest,
    packets: &PacketRuntime,
    pushes: &PushRuntime,
    friends: &mut BTreeMap<u32, FriendEntry>,
    context: &mut OnlineContext<'_>,
) -> Result<Value, AccountActionError> {
    let params = request.params();
    if request.action() == "send_like" {
        let user_id = required_u32(params.get("user_id"))?;
        let uid = directory::friend_uid(user_id, packets, pushes, friends, context).await?;
        let control = friend_like(&uid, optional_u32(params.get("times"), 1)?)
            .map_err(|_error| AccountActionError::BadParameters)?;
        send_control(&control, packets, pushes, context).await?;
        return Ok(json!({}));
    }
    if request.action() == "set_group_add_request" {
        let flag = required_text(params.get("flag"))?;
        let reference = GroupRequestReference::parse(flag)
            .map_err(|_error| AccountActionError::BadParameters)?;
        validate_request_subtype(reference, required_text(params.get("sub_type"))?)?;
        let control = group_request(
            reference.sequence(),
            reference.event_type(),
            reference.group_id(),
            optional_bool(params.get("approve"), true)?,
            params
                .get("reason")
                .and_then(Value::as_str)
                .unwrap_or_default(),
        )
        .map_err(|_error| AccountActionError::BadParameters)?;
        send_control(&control, packets, pushes, context).await?;
        return Ok(json!({}));
    }
    if request.action() == "set_friend_add_request" {
        if params
            .get("remark")
            .and_then(Value::as_str)
            .is_some_and(|value| !value.is_empty())
        {
            return Err(AccountActionError::Unsupported);
        }
        let reference = FriendRequestReference::parse(required_text(params.get("flag"))?)
            .map_err(|_error| AccountActionError::BadParameters)?;
        let records = directory::friend_requests(packets, pushes, context).await?;
        if !records.iter().any(|record| {
            record.is_pending()
                && record.target_uid == context.credential.uid()
                && record.source_uid == reference.source_uid()
        }) {
            return Err(AccountActionError::QqFailure);
        }
        let control = friend_request(
            reference.source_uid(),
            optional_bool(params.get("approve"), true)?,
        )
        .map_err(|_error| AccountActionError::BadParameters)?;
        send_control(&control, packets, pushes, context).await?;
        return Ok(json!({}));
    }
    let group_id = required_u32(params.get("group_id"))?;
    let control = match request.action() {
        "set_group_kick" => {
            let uid = target_uid(group_id, params.get("user_id"), packets, pushes, context).await?;
            group_kick(
                group_id,
                &uid,
                optional_bool(params.get("reject_add_request"), false)?,
                params
                    .get("reason")
                    .and_then(Value::as_str)
                    .unwrap_or_default(),
            )
        }
        "set_group_ban" => {
            let uid = target_uid(group_id, params.get("user_id"), packets, pushes, context).await?;
            group_ban(group_id, &uid, optional_u32(params.get("duration"), 1_800)?)
        }
        "set_group_whole_ban" => {
            group_whole_ban(group_id, optional_bool(params.get("enable"), true)?)
        }
        "set_group_admin" => {
            let uid = target_uid(group_id, params.get("user_id"), packets, pushes, context).await?;
            group_admin(group_id, &uid, optional_bool(params.get("enable"), true)?)
        }
        "set_group_card" => {
            let uid = target_uid(group_id, params.get("user_id"), packets, pushes, context).await?;
            group_card(group_id, &uid, required_text(params.get("card"))?)
        }
        "set_group_name" => group_name(group_id, required_text(params.get("group_name"))?),
        "set_group_leave" => {
            if optional_bool(params.get("is_dismiss"), false)? {
                return Err(AccountActionError::Unsupported);
            }
            group_leave(group_id)
        }
        "set_group_special_title" => {
            let uid = target_uid(group_id, params.get("user_id"), packets, pushes, context).await?;
            group_special_title(group_id, &uid, required_text(params.get("special_title"))?)
        }
        _ => return Err(AccountActionError::ActionNotFound),
    }
    .map_err(|_error| AccountActionError::BadParameters)?;
    send_control(&control, packets, pushes, context).await?;
    Ok(json!({}))
}

fn validate_request_subtype(
    reference: GroupRequestReference,
    subtype: &str,
) -> Result<(), AccountActionError> {
    match (reference.event_type(), subtype) {
        (1 | 22, "add") | (2, "invite") => Ok(()),
        _ => Err(AccountActionError::BadParameters),
    }
}

async fn target_uid(
    group_id: u32,
    user_id: Option<&Value>,
    packets: &PacketRuntime,
    pushes: &PushRuntime,
    context: &mut OnlineContext<'_>,
) -> Result<String, AccountActionError> {
    directory::member_uid(group_id, required_u32(user_id)?, packets, pushes, context).await
}

async fn send_control(
    request: &ControlRequest,
    packets: &PacketRuntime,
    pushes: &PushRuntime,
    context: &mut OnlineContext<'_>,
) -> Result<(), AccountActionError> {
    let reserve = match request.signing_operation() {
        Some(operation) => request_reserve(
            context.ceylith,
            context.account_slot_id,
            OpaqueOperation::numeric(operation),
            request.body(),
        )
        .await
        .map_err(|_error| AccountActionError::QqFailure)?,
        None => Vec::new(),
    };
    let response = packets
        .send_with_reserve(
            PacketContext::for_account(context, pushes.plan()),
            request.command(),
            &reserve,
            request.body(),
        )
        .await
        .map_err(|_error| AccountActionError::QqFailure)?;
    parse_control_response(&response).map_err(|_error| AccountActionError::QqFailure)
}
