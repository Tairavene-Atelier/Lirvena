use std::collections::{BTreeMap, BTreeSet};

use account_api::AccountActionError;
use qq_directory::{
    FriendEntry, GroupEntry, GroupMember, GroupMemberRole, encode_friend_page_request,
    encode_group_list_request, encode_group_member_page_request, parse_friend_page,
    parse_group_list, parse_group_member_page,
};
use serde_json::{Value, json};

use super::packets::{PacketContext, PacketRuntime};
use super::parameters::required_u32;
use super::push::PushRuntime;
use super::runtime::OnlineContext;

pub(super) async fn friend_list(
    packets: &PacketRuntime,
    pushes: &PushRuntime,
    friends: &mut BTreeMap<u32, FriendEntry>,
    context: &mut OnlineContext<'_>,
) -> Result<Value, AccountActionError> {
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

pub(super) async fn refresh_friends(
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
                PacketContext::for_account(context, pushes.plan()),
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

pub(super) async fn group_list(
    packets: &PacketRuntime,
    pushes: &PushRuntime,
    context: &mut OnlineContext<'_>,
) -> Result<Value, AccountActionError> {
    Ok(Value::Array(
        fetch_groups(packets, pushes, context)
            .await?
            .iter()
            .map(group_value)
            .collect(),
    ))
}

pub(super) async fn group_info(
    group_id: Option<&Value>,
    packets: &PacketRuntime,
    pushes: &PushRuntime,
    context: &mut OnlineContext<'_>,
) -> Result<Value, AccountActionError> {
    let group_id = required_u32(group_id)?;
    fetch_groups(packets, pushes, context)
        .await?
        .iter()
        .find(|group| group.group_id == group_id)
        .map(group_value)
        .ok_or(AccountActionError::QqFailure)
}

pub(super) async fn group_member_list(
    group_id: Option<&Value>,
    packets: &PacketRuntime,
    pushes: &PushRuntime,
    context: &mut OnlineContext<'_>,
) -> Result<Value, AccountActionError> {
    let group_id = required_u32(group_id)?;
    Ok(Value::Array(
        fetch_group_members(group_id, packets, pushes, context)
            .await?
            .iter()
            .map(|member| member_value(group_id, member))
            .collect(),
    ))
}

pub(super) async fn group_member_info(
    group_id: Option<&Value>,
    user_id: Option<&Value>,
    packets: &PacketRuntime,
    pushes: &PushRuntime,
    context: &mut OnlineContext<'_>,
) -> Result<Value, AccountActionError> {
    let group_id = required_u32(group_id)?;
    let user_id = required_u32(user_id)?;
    fetch_group_members(group_id, packets, pushes, context)
        .await?
        .iter()
        .find(|member| member.uin == user_id)
        .map(|member| member_value(group_id, member))
        .ok_or(AccountActionError::QqFailure)
}

async fn fetch_groups(
    packets: &PacketRuntime,
    pushes: &PushRuntime,
    context: &mut OnlineContext<'_>,
) -> Result<Vec<GroupEntry>, AccountActionError> {
    let body = encode_group_list_request();
    let response = packets
        .send_with_reserve(
            PacketContext::for_account(context, pushes.plan()),
            "OidbSvcTrpcTcp.0xfe5_2",
            &[],
            &body,
        )
        .await
        .map_err(|_error| AccountActionError::QqFailure)?;
    parse_group_list(&response).map_err(|_error| AccountActionError::QqFailure)
}

async fn fetch_group_members(
    group_id: u32,
    packets: &PacketRuntime,
    pushes: &PushRuntime,
    context: &mut OnlineContext<'_>,
) -> Result<Vec<GroupMember>, AccountActionError> {
    const MAX_PAGES: usize = 64;
    const MAX_MEMBERS: usize = 100_000;
    let mut members = Vec::new();
    let mut seen_uins = BTreeSet::new();
    let mut seen_tokens = BTreeSet::new();
    let mut token = None;
    for _page_number in 0..MAX_PAGES {
        let body = encode_group_member_page_request(group_id, token.as_deref())
            .map_err(|_error| AccountActionError::BadParameters)?;
        let response = packets
            .send_with_reserve(
                PacketContext::for_account(context, pushes.plan()),
                "OidbSvcTrpcTcp.0xfe7_3",
                &[],
                &body,
            )
            .await
            .map_err(|_error| AccountActionError::QqFailure)?;
        let page =
            parse_group_member_page(&response).map_err(|_error| AccountActionError::QqFailure)?;
        for member in page.members {
            if members.len() >= MAX_MEMBERS || !seen_uins.insert(member.uin) {
                return Err(AccountActionError::QqFailure);
            }
            members.push(member);
        }
        match page.next_token {
            Some(next) if seen_tokens.insert(next.clone()) => token = Some(next),
            Some(_) => return Err(AccountActionError::QqFailure),
            None => return Ok(members),
        }
    }
    Err(AccountActionError::QqFailure)
}

fn group_value(group: &GroupEntry) -> Value {
    json!({
        "group_id": group.group_id,
        "group_name": group.group_name,
        "member_count": group.member_count,
        "max_member_count": group.max_member_count,
    })
}

fn member_value(group_id: u32, member: &GroupMember) -> Value {
    let role = match member.role {
        GroupMemberRole::Member => "member",
        GroupMemberRole::Owner => "owner",
        GroupMemberRole::Admin => "admin",
    };
    json!({
        "group_id": group_id,
        "user_id": member.uin,
        "nickname": member.nickname,
        "card": member.card,
        "sex": "unknown",
        "age": 0,
        "area": "",
        "join_time": member.joined_at,
        "last_sent_time": member.last_message_at,
        "level": member.level.to_string(),
        "role": role,
        "unfriendly": false,
        "title": member.special_title,
        "title_expire_time": 0,
        "card_changeable": false,
        "shut_up_timestamp": member.muted_until,
    })
}
