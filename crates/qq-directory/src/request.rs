use std::collections::BTreeSet;

use prost::Message;
use qq_wire::{decode_oidb_response, encode_oidb_request};

const MAX_REQUESTS: usize = 64;
const MAX_UID_BYTES: usize = 128;
const MAX_TEXT_BYTES: usize = 4_096;

/// Opaque group-request directory codec error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GroupRequestDirectoryError;

impl core::fmt::Display for GroupRequestDirectoryError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("QQ group request directory data is invalid")
    }
}

impl std::error::Error for GroupRequestDirectoryError {}

/// Evidence-backed group request kind.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GroupRequestKind {
    /// A user asked to join a group.
    Join,
    /// The current account was invited to a group.
    SelfInvitation,
    /// A group member invited another user.
    Invitation,
}

/// One validated group request recovered from QQ's request directory.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroupRequestRecord {
    /// Server-issued request sequence.
    pub sequence: u64,
    /// Evidence-backed request kind.
    pub kind: GroupRequestKind,
    /// Opaque QQ request state retained for diagnostics.
    pub state: u32,
    /// Numeric QQ group identifier.
    pub group_id: u32,
    /// Current Linux NT UID of the target user.
    pub target_uid: String,
    /// Current Linux NT UID of the inviter when supplied.
    pub inviter_uid: Option<String>,
    /// Current Linux NT UID of the operator when supplied.
    pub operator_uid: Option<String>,
    /// Bounded request comment.
    pub comment: String,
}

impl GroupRequestRecord {
    /// Returns whether QQ marks this request as waiting for an operator decision.
    #[must_use]
    pub const fn is_pending(&self) -> bool {
        self.state == 1
    }
}

/// Encodes the bounded recent group-request query.
///
/// # Errors
///
/// Returns an error only if the shared OIDB envelope rejects the compiled request.
pub fn encode_group_request_list_request() -> Result<Vec<u8>, GroupRequestDirectoryError> {
    let body = RequestListQuery {
        count: 20,
        field2: 0,
    }
    .encode_to_vec();
    encode_oidb_request(0x10c0, 1, &body, 0).map_err(|_error| GroupRequestDirectoryError)
}

/// Parses one successful bounded group-request list response.
///
/// # Errors
///
/// Returns an error for rejected, malformed, excessive, incomplete, or unsafe data.
pub fn parse_group_request_list(
    input: &[u8],
) -> Result<Vec<GroupRequestRecord>, GroupRequestDirectoryError> {
    let outer = decode_oidb_response(input).map_err(|_error| GroupRequestDirectoryError)?;
    if outer.error_code() != 0 {
        return Err(GroupRequestDirectoryError);
    }
    let response =
        RequestListResponse::decode(outer.body()).map_err(|_error| GroupRequestDirectoryError)?;
    if response.requests.len() > MAX_REQUESTS {
        return Err(GroupRequestDirectoryError);
    }
    let records = response
        .requests
        .into_iter()
        .map(validate_record)
        .collect::<Result<Vec<_>, _>>()?;
    let records = records.into_iter().flatten().collect::<Vec<_>>();
    let mut sequences = BTreeSet::new();
    if records
        .iter()
        .any(|record| !sequences.insert(record.sequence))
    {
        return Err(GroupRequestDirectoryError);
    }
    Ok(records)
}

fn validate_record(
    record: RequestRecordWire,
) -> Result<Option<GroupRequestRecord>, GroupRequestDirectoryError> {
    let kind = match record.event_type {
        1 => GroupRequestKind::Join,
        2 => GroupRequestKind::SelfInvitation,
        22 => GroupRequestKind::Invitation,
        _ => return Ok(None),
    };
    let group_id = record.group.ok_or(GroupRequestDirectoryError)?.group_id;
    let target_uid = record.target.ok_or(GroupRequestDirectoryError)?.uid;
    if record.sequence == 0 || group_id == 0 {
        return Err(GroupRequestDirectoryError);
    }
    validate_uid(&target_uid)?;
    let inviter_uid = optional_uid(record.inviter)?;
    let operator_uid = optional_uid(record.operator)?;
    validate_text(&record.comment)?;
    Ok(Some(GroupRequestRecord {
        sequence: record.sequence,
        kind,
        state: record.state,
        group_id,
        target_uid,
        inviter_uid,
        operator_uid,
        comment: record.comment,
    }))
}

fn optional_uid(user: Option<UserWire>) -> Result<Option<String>, GroupRequestDirectoryError> {
    user.map(|value| {
        validate_uid(&value.uid)?;
        Ok(value.uid)
    })
    .transpose()
}

fn validate_uid(uid: &str) -> Result<(), GroupRequestDirectoryError> {
    if uid.is_empty() || uid.len() > MAX_UID_BYTES || uid.chars().any(char::is_control) {
        Err(GroupRequestDirectoryError)
    } else {
        Ok(())
    }
}

fn validate_text(value: &str) -> Result<(), GroupRequestDirectoryError> {
    if value.len() > MAX_TEXT_BYTES || value.chars().any(char::is_control) {
        Err(GroupRequestDirectoryError)
    } else {
        Ok(())
    }
}

#[derive(Clone, Copy, PartialEq, Message)]
struct RequestListQuery {
    #[prost(uint32, tag = "1")]
    count: u32,
    #[prost(uint32, tag = "2")]
    field2: u32,
}

#[derive(Clone, PartialEq, Message)]
struct RequestListResponse {
    #[prost(message, repeated, tag = "1")]
    requests: Vec<RequestRecordWire>,
}

#[derive(Clone, PartialEq, Message)]
struct RequestRecordWire {
    #[prost(uint64, tag = "1")]
    sequence: u64,
    #[prost(uint32, tag = "2")]
    event_type: u32,
    #[prost(uint32, tag = "3")]
    state: u32,
    #[prost(message, optional, tag = "4")]
    group: Option<GroupWire>,
    #[prost(message, optional, tag = "5")]
    target: Option<UserWire>,
    #[prost(message, optional, tag = "6")]
    inviter: Option<UserWire>,
    #[prost(message, optional, tag = "7")]
    operator: Option<UserWire>,
    #[prost(string, tag = "10")]
    comment: String,
}

#[derive(Clone, PartialEq, Message)]
struct GroupWire {
    #[prost(uint32, tag = "1")]
    group_id: u32,
    #[prost(string, tag = "2")]
    name: String,
}

#[derive(Clone, PartialEq, Message)]
struct UserWire {
    #[prost(string, tag = "1")]
    uid: String,
    #[prost(string, tag = "2")]
    name: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_list_preserves_action_reference_and_comment()
    -> Result<(), Box<dyn std::error::Error>> {
        let inner = RequestListResponse {
            requests: vec![RequestRecordWire {
                sequence: 77,
                event_type: 1,
                state: 1,
                group: Some(GroupWire {
                    group_id: 12_345,
                    name: "group".to_owned(),
                }),
                target: Some(UserWire {
                    uid: "u_target".to_owned(),
                    name: "target".to_owned(),
                }),
                inviter: None,
                operator: None,
                comment: "hello".to_owned(),
            }],
        }
        .encode_to_vec();
        let response = qq_wire::encode_oidb_request(0x10c0, 1, &inner, 0)?;
        let records = parse_group_request_list(&response)?;
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].sequence, 77);
        assert_eq!(records[0].kind, GroupRequestKind::Join);
        assert_eq!(records[0].comment, "hello");
        assert!(records[0].is_pending());
        Ok(())
    }

    #[test]
    fn unknown_request_kind_is_not_exposed_as_actionable() -> Result<(), Box<dyn std::error::Error>>
    {
        let inner = RequestListResponse {
            requests: vec![RequestRecordWire {
                sequence: 1,
                event_type: 999,
                state: 0,
                group: Some(GroupWire {
                    group_id: 1,
                    name: String::new(),
                }),
                target: Some(UserWire {
                    uid: "u".to_owned(),
                    name: String::new(),
                }),
                inviter: None,
                operator: None,
                comment: String::new(),
            }],
        }
        .encode_to_vec();
        let response = qq_wire::encode_oidb_request(0x10c0, 1, &inner, 0)?;
        assert!(parse_group_request_list(&response)?.is_empty());
        Ok(())
    }
}
