use std::collections::{BTreeMap, BTreeSet};

use account_api::{
    AccountActionError, AccountIdentity, FriendRequestReference, GroupRequestKind as ApiKind,
    GroupRequestReference, ResolvedFriendRequest, ResolvedGroupRequest,
};
use qq_directory::{FriendRequestRecord, GroupRequestKind as DirectoryKind, GroupRequestRecord};
use qq_message::{FriendRequestSignal, GroupRequestSignal};
use serde_json::{Map, Value, json};

use super::{directory, packets::PacketRuntime, push::PushRuntime, runtime::OnlineContext};
use crate::support::now_seconds;

pub(super) async fn list_group_requests(
    identity: &AccountIdentity,
    packets: &PacketRuntime,
    pushes: &PushRuntime,
    context: &mut OnlineContext<'_>,
) -> Result<Value, AccountActionError> {
    let records = directory::group_requests(packets, pushes, context).await?;
    let uids = records
        .iter()
        .flat_map(|record| {
            [
                Some(record.target_uid.as_str()),
                record.inviter_uid.as_deref(),
            ]
        })
        .flatten()
        .collect::<BTreeSet<_>>();
    let mut resolved = BTreeMap::new();
    for uid in uids {
        resolved.insert(
            uid.to_owned(),
            u64::from(directory::uid_uin(uid, packets, pushes, context).await?),
        );
    }
    let observed_at = u64::from(now_seconds().map_err(|_error| AccountActionError::QqFailure)?);
    project_group_records(identity, &records, &resolved, observed_at)
}

pub(super) async fn list_friend_requests(
    identity: &AccountIdentity,
    packets: &PacketRuntime,
    pushes: &PushRuntime,
    context: &mut OnlineContext<'_>,
) -> Result<Value, AccountActionError> {
    let records = directory::friend_requests(packets, pushes, context).await?;
    if records
        .iter()
        .any(|record| record.target_uid != context.credential.uid())
    {
        return Err(AccountActionError::QqFailure);
    }
    let uids = records
        .iter()
        .map(|record| record.source_uid.as_str())
        .collect::<BTreeSet<_>>();
    let mut resolved = BTreeMap::new();
    for uid in uids {
        resolved.insert(
            uid.to_owned(),
            u64::from(directory::uid_uin(uid, packets, pushes, context).await?),
        );
    }
    project_friend_records(identity, &records, &resolved)
}

fn project_group_records(
    identity: &AccountIdentity,
    records: &[GroupRequestRecord],
    resolved: &BTreeMap<String, u64>,
    observed_at: u64,
) -> Result<Value, AccountActionError> {
    records
        .iter()
        .map(|record| {
            let (sub_type, subject_uid, inviter_uid) = match record.kind {
                DirectoryKind::Join => ("add", record.target_uid.as_str(), None),
                DirectoryKind::Invitation => (
                    "add",
                    record.target_uid.as_str(),
                    record.inviter_uid.as_deref(),
                ),
                DirectoryKind::SelfInvitation => (
                    "invite",
                    record
                        .inviter_uid
                        .as_deref()
                        .ok_or(AccountActionError::QqFailure)?,
                    record.inviter_uid.as_deref(),
                ),
            };
            let event_type = event_type(record.kind);
            let reference =
                GroupRequestReference::new(record.sequence, event_type, record.group_id)
                    .map_err(|_error| AccountActionError::QqFailure)?;
            let mut object = Map::from_iter([
                ("time".to_owned(), json!(observed_at)),
                ("self_id".to_owned(), json!(identity.qq_id())),
                ("post_type".to_owned(), json!("request")),
                ("request_type".to_owned(), json!("group")),
                ("sub_type".to_owned(), json!(sub_type)),
                (
                    "user_id".to_owned(),
                    json!(resolved_uid(resolved, subject_uid)?),
                ),
                ("group_id".to_owned(), json!(record.group_id)),
                ("comment".to_owned(), json!(record.comment)),
                ("flag".to_owned(), json!(reference.flag())),
            ]);
            if let Some(uid) = inviter_uid {
                object.insert("invitor_id".to_owned(), json!(resolved_uid(resolved, uid)?));
            }
            Ok(Value::Object(object))
        })
        .collect::<Result<Vec<_>, _>>()
        .map(Value::Array)
}

fn project_friend_records(
    identity: &AccountIdentity,
    records: &[FriendRequestRecord],
    resolved: &BTreeMap<String, u64>,
) -> Result<Value, AccountActionError> {
    records
        .iter()
        .map(|record| {
            let reference = FriendRequestReference::new(record.source_uid.clone())
                .map_err(|_error| AccountActionError::QqFailure)?;
            Ok(json!({
                "time": record.timestamp,
                "self_id": identity.qq_id(),
                "post_type": "request",
                "request_type": "friend",
                "user_id": resolved_uid(resolved, &record.source_uid)?,
                "comment": record.comment,
                "source": record.source,
                "flag": reference.flag(),
            }))
        })
        .collect::<Result<Vec<_>, _>>()
        .map(Value::Array)
}

fn resolved_uid(resolved: &BTreeMap<String, u64>, uid: &str) -> Result<u64, AccountActionError> {
    resolved
        .get(uid)
        .copied()
        .filter(|value| *value != 0)
        .ok_or(AccountActionError::QqFailure)
}

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

#[cfg(test)]
mod tests {
    use account_api::{AccountIdentity, GroupRequestReference};
    use account_runtime::AccountLocalId;
    use qq_directory::{FriendRequestRecord, GroupRequestKind, GroupRequestRecord};
    use serde_json::Value;

    use super::{BTreeMap, project_friend_records, project_group_records};

    #[test]
    fn group_request_list_keeps_actionable_flags_and_inviter()
    -> Result<(), Box<dyn std::error::Error>> {
        let identity = identity()?;
        let records = [GroupRequestRecord {
            sequence: 77,
            kind: GroupRequestKind::Invitation,
            state: 1,
            group_id: 12_345,
            target_uid: "u_target".to_owned(),
            inviter_uid: Some("u_inviter".to_owned()),
            operator_uid: None,
            comment: "hello".to_owned(),
        }];
        let resolved = BTreeMap::from([("u_target".to_owned(), 42), ("u_inviter".to_owned(), 43)]);
        let Value::Array(projected) = project_group_records(&identity, &records, &resolved, 99)?
        else {
            return Err("expected request array".into());
        };
        assert_eq!(projected[0]["sub_type"], "add");
        assert_eq!(projected[0]["user_id"], 42);
        assert_eq!(projected[0]["invitor_id"], 43);
        let flag = projected[0]["flag"].as_str().ok_or("missing flag")?;
        assert_eq!(GroupRequestReference::parse(flag)?.event_type(), 22);
        Ok(())
    }

    #[test]
    fn friend_request_list_uses_the_same_versioned_flag() -> Result<(), Box<dyn std::error::Error>>
    {
        let identity = identity()?;
        let records = [FriendRequestRecord {
            target_uid: "u_self".to_owned(),
            source_uid: "u_friend".to_owned(),
            state: 1,
            timestamp: 99,
            comment: "hello".to_owned(),
            source: "search".to_owned(),
        }];
        let resolved = BTreeMap::from([("u_friend".to_owned(), 42)]);
        let Value::Array(projected) = project_friend_records(&identity, &records, &resolved)?
        else {
            return Err("expected request array".into());
        };
        assert_eq!(projected[0]["request_type"], "friend");
        assert_eq!(projected[0]["user_id"], 42);
        let flag = projected[0]["flag"].as_str().ok_or("missing flag")?;
        assert_eq!(
            account_api::FriendRequestReference::parse(flag)?.source_uid(),
            "u_friend"
        );
        Ok(())
    }

    fn identity() -> Result<AccountIdentity, account_api::EventHubError> {
        AccountIdentity::new(
            AccountLocalId::from_bytes([1; 16]),
            10_001,
            "self".to_owned(),
        )
    }
}
