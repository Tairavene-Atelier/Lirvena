use account_api::{
    AccountActionError, AccountIdentity, FriendRequestReference, GroupRequestKind as ApiKind,
    GroupRequestReference, ResolvedFriendRequest, ResolvedGroupRequest,
};
use qq_directory::{GroupRequestKind as DirectoryKind, GroupRequestRecord};
use qq_message::{FriendRequestSignal, GroupRequestSignal};

use super::{directory, packets::PacketRuntime, push::PushRuntime, runtime::OnlineContext};

pub(super) async fn resolve_group_request(
    identity: &AccountIdentity,
    packets: &PacketRuntime,
    pushes: &PushRuntime,
    signal: GroupRequestSignal,
    occurred_at: u64,
    context: &mut OnlineContext<'_>,
) -> Result<Option<ResolvedGroupRequest>, AccountActionError> {
    let records = directory::group_requests(packets, pushes, context).await?;
    let record = records
        .iter()
        .filter(|record| record.is_pending() && signal_matches(&signal, record))
        .max_by_key(|record| record.sequence);
    let Some(record) = record else {
        return Ok(None);
    };
    let (kind, subject_reference, inviter_reference) =
        projection_fields(record).ok_or(AccountActionError::QqFailure)?;
    let resolved_subject =
        u64::from(directory::uid_uin(subject_reference, packets, pushes, context).await?);
    let resolved_inviter = match inviter_reference {
        Some(uid) if uid == subject_reference => Some(resolved_subject),
        Some(uid) => Some(u64::from(
            directory::uid_uin(uid, packets, pushes, context).await?,
        )),
        None => None,
    };
    let reference =
        GroupRequestReference::new(record.sequence, event_type(record.kind), record.group_id)
            .map_err(|_error| AccountActionError::QqFailure)?;
    ResolvedGroupRequest::new(
        identity.clone(),
        reference,
        kind,
        resolved_subject,
        resolved_inviter,
        record.comment.clone(),
        occurred_at,
    )
    .map(Some)
    .map_err(|_error| AccountActionError::QqFailure)
}

pub(super) async fn resolve_friend_request(
    identity: &AccountIdentity,
    packets: &PacketRuntime,
    pushes: &PushRuntime,
    signal: FriendRequestSignal,
    context: &mut OnlineContext<'_>,
) -> Result<Option<ResolvedFriendRequest>, AccountActionError> {
    let records = directory::friend_requests(packets, pushes, context).await?;
    let record = records
        .iter()
        .filter(|record| {
            record.is_pending()
                && record.target_uid == context.credential.uid()
                && record.source_uid == signal.source_uid()
        })
        .max_by_key(|record| record.timestamp);
    let Some(record) = record else {
        return Ok(None);
    };
    let user_id =
        u64::from(directory::uid_uin(&record.source_uid, packets, pushes, context).await?);
    let reference = FriendRequestReference::new(record.source_uid.clone())
        .map_err(|_error| AccountActionError::QqFailure)?;
    ResolvedFriendRequest::new(
        identity.clone(),
        reference,
        user_id,
        record.comment.clone(),
        u64::from(record.timestamp),
    )
    .map(Some)
    .map_err(|_error| AccountActionError::QqFailure)
}

fn signal_matches(signal: &GroupRequestSignal, record: &GroupRequestRecord) -> bool {
    match signal {
        GroupRequestSignal::Join {
            group_id,
            target_uid,
        } => {
            record.kind == DirectoryKind::Join
                && record.group_id == *group_id
                && record.target_uid == *target_uid
        }
        GroupRequestSignal::Invitation {
            group_id,
            target_uid,
            inviter_uid,
        } => {
            record.kind == DirectoryKind::Invitation
                && record.group_id == *group_id
                && record.target_uid == *target_uid
                && record.inviter_uid.as_deref() == Some(inviter_uid)
        }
        GroupRequestSignal::SelfInvitation {
            group_id,
            inviter_uid,
        } => {
            record.kind == DirectoryKind::SelfInvitation
                && record.group_id == *group_id
                && record.inviter_uid.as_deref() == Some(inviter_uid)
        }
    }
}

fn projection_fields(record: &GroupRequestRecord) -> Option<(ApiKind, &str, Option<&str>)> {
    match record.kind {
        DirectoryKind::Join => Some((ApiKind::Join, &record.target_uid, None)),
        DirectoryKind::Invitation => Some((
            ApiKind::Invitation,
            &record.target_uid,
            record.inviter_uid.as_deref(),
        )),
        DirectoryKind::SelfInvitation => record
            .inviter_uid
            .as_deref()
            .map(|inviter| (ApiKind::SelfInvitation, inviter, Some(inviter))),
    }
}

const fn event_type(kind: DirectoryKind) -> u32 {
    match kind {
        DirectoryKind::Join => 1,
        DirectoryKind::SelfInvitation => 2,
        DirectoryKind::Invitation => 22,
    }
}
