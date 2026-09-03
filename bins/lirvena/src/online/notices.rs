use account_api::{
    AccountIdentity, ResolvedGroupNotice, ResolvedGroupNoticeKind, ResolvedGroupReaction,
};
use qq_message::{GroupNotice, GroupReaction, MemberDecreaseKind};

use super::{
    directory, message_registry::MessageRegistry, packets::PacketRuntime, push::PushRuntime,
    runtime::OnlineContext,
};

pub(super) async fn resolve_group_reaction(
    identity: &AccountIdentity,
    packets: &PacketRuntime,
    pushes: &PushRuntime,
    messages: &mut MessageRegistry,
    reaction: GroupReaction,
    occurred_at: u64,
    context: &mut OnlineContext<'_>,
) -> Option<ResolvedGroupReaction> {
    let message_id = messages
        .find_group_message_id(reaction.group_id(), u64::from(reaction.sequence()))
        .ok()??;
    let operator_id = if let Some(value) = parse_uid(reaction.operator_uid()) {
        value
    } else {
        let members = directory::group_members(reaction.group_id(), packets, pushes, context)
            .await
            .ok()?;
        resolve_uid(reaction.operator_uid(), Some(&members))?
    };
    ResolvedGroupReaction::new(
        identity.clone(),
        u64::from(reaction.group_id()),
        message_id,
        operator_id,
        reaction.is_add(),
        reaction.code().to_owned(),
        reaction.count(),
        occurred_at,
    )
    .ok()
}

pub(super) async fn resolve_group_notice(
    identity: &AccountIdentity,
    packets: &PacketRuntime,
    pushes: &PushRuntime,
    notice: GroupNotice,
    occurred_at: u64,
    context: &mut OnlineContext<'_>,
) -> Option<ResolvedGroupNotice> {
    let (group_id, member_uid, operator_uid, kind) = notice_parts(notice);
    let needs_members = parse_uid(&member_uid).is_none()
        || operator_uid
            .as_deref()
            .is_some_and(|uid| parse_uid(uid).is_none());
    let members = if needs_members {
        directory::group_members(group_id, packets, pushes, context)
            .await
            .ok()
    } else {
        None
    };
    let user_id = match kind {
        ResolvedGroupNoticeKind::MemberDecrease(MemberDecreaseKind::KickMe) => identity.qq_id(),
        _ => resolve_uid(&member_uid, members.as_deref())?,
    };
    let resolved_operator = match operator_uid.as_deref() {
        Some(uid) => Some(resolve_uid(uid, members.as_deref())?),
        None => None,
    };
    ResolvedGroupNotice::new(
        identity.clone(),
        u64::from(group_id),
        user_id,
        resolved_operator,
        kind,
        occurred_at,
    )
    .ok()
}

fn notice_parts(notice: GroupNotice) -> (u32, String, Option<String>, ResolvedGroupNoticeKind) {
    match notice {
        GroupNotice::Administrator {
            group_id,
            member_uid,
            enabled,
        } => (
            group_id,
            member_uid,
            None,
            if enabled {
                ResolvedGroupNoticeKind::AdministratorSet
            } else {
                ResolvedGroupNoticeKind::AdministratorUnset
            },
        ),
        GroupNotice::MemberIncrease {
            group_id,
            member_uid,
            operator_uid,
            kind,
        } => (
            group_id,
            member_uid,
            operator_uid,
            ResolvedGroupNoticeKind::MemberIncrease(kind),
        ),
        GroupNotice::MemberDecrease {
            group_id,
            member_uid,
            operator_uid,
            kind,
        } => (
            group_id,
            member_uid,
            operator_uid,
            ResolvedGroupNoticeKind::MemberDecrease(kind),
        ),
    }
}

fn resolve_uid(uid: &str, members: Option<&[qq_directory::GroupMember]>) -> Option<u64> {
    parse_uid(uid).or_else(|| {
        members?
            .iter()
            .find(|member| member.uid == uid)
            .map(|member| u64::from(member.uin))
    })
}

fn parse_uid(uid: &str) -> Option<u64> {
    uid.parse::<u64>().ok().filter(|value| *value != 0)
}
