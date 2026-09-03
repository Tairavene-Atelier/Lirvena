use std::collections::BTreeMap;

use account_api::{AccountActionError, AccountActionRequest, AccountIdentity};
use adapter_onebot::{IdFormat, project_history_message};
use qq_directory::FriendEntry;
use qq_message::{
    FRIEND_HISTORY_ROUTE, GROUP_HISTORY_ROUTE, decode_friend_history_response,
    decode_group_history_response, encode_friend_history_request, encode_group_history_request,
};
use serde_json::{Value, json};

use super::message_registry::MessageRegistry;
use super::packets::{PacketContext, PacketRuntime};
use super::parameters::{optional_u32, required_u32};
use super::push::PushRuntime;
use super::runtime::OnlineContext;
use crate::support::now_ms;

const DEFAULT_HISTORY_COUNT: u32 = 20;
const MAX_HISTORY_COUNT: u32 = 100;

pub(super) async fn friend(
    request: &AccountActionRequest,
    identity: &AccountIdentity,
    packets: &PacketRuntime,
    pushes: &PushRuntime,
    friends: &mut BTreeMap<u32, FriendEntry>,
    messages: &mut MessageRegistry,
    context: &mut OnlineContext<'_>,
) -> Result<Value, AccountActionError> {
    let user_id = required_u32(request.params().get("user_id"))?;
    let message_id = required_u32(request.params().get("message_id"))?;
    let count = history_count(request)?;
    let peer_uid = super::directory::friend_uid(user_id, packets, pushes, friends, context).await?;
    let record = messages
        .get(message_id)
        .map_err(|_error| AccountActionError::QqFailure)?
        .ok_or(AccountActionError::QqFailure)?;
    let timestamp = friend_anchor(record.recall(), &peer_uid)?;
    let payload = encode_friend_history_request(&peer_uid, timestamp, count)
        .map_err(|_error| AccountActionError::BadParameters)?;
    let response = packets
        .send_with_reserve(
            PacketContext::for_account(context, pushes.plan()),
            FRIEND_HISTORY_ROUTE,
            &[],
            &payload,
        )
        .await
        .map_err(|_error| AccountActionError::QqFailure)?;
    let self_uin =
        u32::try_from(identity.qq_id()).map_err(|_error| AccountActionError::QqFailure)?;
    let history =
        decode_friend_history_response(&response, &peer_uid, user_id, self_uin, timestamp, count)
            .map_err(|_error| AccountActionError::QqFailure)?;
    retain_and_project(history, identity, messages)
}

pub(super) async fn group(
    request: &AccountActionRequest,
    identity: &AccountIdentity,
    packets: &PacketRuntime,
    pushes: &PushRuntime,
    messages: &mut MessageRegistry,
    context: &mut OnlineContext<'_>,
) -> Result<Value, AccountActionError> {
    let group_id = required_u32(request.params().get("group_id"))?;
    let message_id = required_u32(request.params().get("message_id"))?;
    let count = history_count(request)?;
    let record = messages
        .get(message_id)
        .map_err(|_error| AccountActionError::QqFailure)?
        .ok_or(AccountActionError::QqFailure)?;
    let (start, end) = group_interval(record.recall(), group_id, count)?;
    let payload = encode_group_history_request(group_id, start, end)
        .map_err(|_error| AccountActionError::BadParameters)?;
    let response = packets
        .send_with_reserve(
            PacketContext::for_account(context, pushes.plan()),
            GROUP_HISTORY_ROUTE,
            &[],
            &payload,
        )
        .await
        .map_err(|_error| AccountActionError::QqFailure)?;
    let history = decode_group_history_response(&response, group_id, start, end)
        .map_err(|_error| AccountActionError::QqFailure)?;
    retain_and_project(history, identity, messages)
}

fn retain_and_project(
    history: Vec<qq_message::HistoricalMessage>,
    identity: &AccountIdentity,
    messages: &mut MessageRegistry,
) -> Result<Value, AccountActionError> {
    let inserted_at = now_ms().map_err(|_error| AccountActionError::QqFailure)?;
    let projected = history
        .into_iter()
        .map(|historical| {
            let (envelope, rich_text) = historical.into_parts();
            let message = messages
                .retain_decoded(identity, envelope, rich_text, inserted_at)
                .map_err(|_error| AccountActionError::QqFailure)?;
            project_history_message(&message, IdFormat::Number)
                .map_err(|_error| AccountActionError::QqFailure)
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(json!({"messages": projected}))
}

fn history_count(request: &AccountActionRequest) -> Result<u32, AccountActionError> {
    let count = optional_u32(request.params().get("count"), DEFAULT_HISTORY_COUNT)?;
    if count == 0 || count > MAX_HISTORY_COUNT {
        Err(AccountActionError::BadParameters)
    } else {
        Ok(count)
    }
}

fn group_interval(
    target: &account_message_store::RecallTarget,
    group_id: u32,
    count: u32,
) -> Result<(u32, u32), AccountActionError> {
    let end = match target {
        account_message_store::RecallTarget::Group {
            group_code,
            sequence,
            ..
        } if *group_code == group_id => {
            u32::try_from(*sequence).map_err(|_error| AccountActionError::QqFailure)?
        }
        _ => return Err(AccountActionError::QqFailure),
    };
    let start = if end > count { end - count + 1 } else { 0 };
    Ok((start, end))
}

fn friend_anchor(
    target: &account_message_store::RecallTarget,
    peer_uid: &str,
) -> Result<u32, AccountActionError> {
    match target {
        account_message_store::RecallTarget::Private { uid, timestamp, .. }
            if uid == peer_uid && *timestamp != 0 =>
        {
            Ok(*timestamp)
        }
        _ => Err(AccountActionError::QqFailure),
    }
}

#[cfg(test)]
mod tests {
    use account_message_store::RecallTarget;

    use super::{friend_anchor, group_interval};

    #[test]
    fn interval_requires_the_retained_same_group_sequence() {
        let target = RecallTarget::Group {
            group_code: 88,
            sequence: 100,
            random: None,
        };
        assert_eq!(group_interval(&target, 88, 20), Ok((81, 100)));
        assert_eq!(group_interval(&target, 88, 100), Ok((0, 100)));
        assert!(group_interval(&target, 89, 20).is_err());
        assert!(group_interval(&RecallTarget::Unavailable, 88, 20).is_err());
    }

    #[test]
    fn friend_anchor_requires_the_retained_same_peer() {
        let target = RecallTarget::Private {
            uid: "u_peer".to_owned(),
            peer_uin: Some(42),
            sequence: 100,
            client_sequence: 101,
            random: 102,
            timestamp: 103,
        };
        assert_eq!(friend_anchor(&target, "u_peer"), Ok(103));
        assert!(friend_anchor(&target, "u_other").is_err());
        assert!(friend_anchor(&RecallTarget::Unavailable, "u_peer").is_err());
    }
}
