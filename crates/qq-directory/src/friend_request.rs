use std::collections::BTreeSet;

use prost::Message;
use qq_wire::{decode_oidb_response, encode_oidb_request};

const MAX_REQUESTS: usize = 64;
const MAX_UID_BYTES: usize = 128;
const MAX_TEXT_BYTES: usize = 4_096;

/// Opaque friend-request directory codec error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FriendRequestDirectoryError;

impl core::fmt::Display for FriendRequestDirectoryError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("QQ friend request directory data is invalid")
    }
}

impl std::error::Error for FriendRequestDirectoryError {}

/// One validated friend request recovered from QQ's request directory.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FriendRequestRecord {
    /// Current Linux NT UID of the receiving account.
    pub target_uid: String,
    /// Current Linux NT UID of the applicant.
    pub source_uid: String,
    /// Opaque QQ request state retained for admission checks.
    pub state: u32,
    /// QQ-supplied Unix event time in seconds.
    pub timestamp: u32,
    /// Bounded verification comment.
    pub comment: String,
    /// Bounded request-source label.
    pub source: String,
}

impl FriendRequestRecord {
    /// Returns whether QQ marks this request as waiting for a decision.
    #[must_use]
    pub const fn is_pending(&self) -> bool {
        self.state == 1
    }
}

/// Encodes the bounded recent friend-request query for the authenticated account.
///
/// # Errors
///
/// Returns an error for an unsafe account UID or rejected shared envelope.
pub fn encode_friend_request_list_request(
    self_uid: &str,
) -> Result<Vec<u8>, FriendRequestDirectoryError> {
    validate_uid(self_uid)?;
    let body = FriendRequestQuery {
        field1: 1,
        field3: 6,
        self_uid: self_uid.to_owned(),
        field5: 0,
        count: 80,
        field8: 2,
        field9: 0,
        field12: 1,
        field22: 1,
    }
    .encode_to_vec();
    encode_oidb_request(0x05cf, 11, &body, 0).map_err(|_error| FriendRequestDirectoryError)
}

/// Parses one successful bounded friend-request list response.
///
/// # Errors
///
/// Returns an error for rejected, malformed, excessive, duplicate, incomplete, or unsafe data.
pub fn parse_friend_request_list(
    input: &[u8],
) -> Result<Vec<FriendRequestRecord>, FriendRequestDirectoryError> {
    let outer = decode_oidb_response(input).map_err(|_error| FriendRequestDirectoryError)?;
    if outer.error_code() != 0 {
        return Err(FriendRequestDirectoryError);
    }
    let requests = FriendRequestResponse::decode(outer.body())
        .map_err(|_error| FriendRequestDirectoryError)?
        .info
        .ok_or(FriendRequestDirectoryError)?
        .requests;
    if requests.len() > MAX_REQUESTS {
        return Err(FriendRequestDirectoryError);
    }
    let mut seen = BTreeSet::new();
    requests
        .into_iter()
        .map(|request| {
            validate_uid(&request.target_uid)?;
            validate_uid(&request.source_uid)?;
            validate_text(&request.comment)?;
            validate_text(&request.source)?;
            if !seen.insert((request.source_uid.clone(), request.timestamp)) {
                return Err(FriendRequestDirectoryError);
            }
            Ok(FriendRequestRecord {
                target_uid: request.target_uid,
                source_uid: request.source_uid,
                state: request.state,
                timestamp: request.timestamp,
                comment: request.comment,
                source: request.source,
            })
        })
        .collect()
}

fn validate_uid(value: &str) -> Result<(), FriendRequestDirectoryError> {
    if value.is_empty() || value.len() > MAX_UID_BYTES || value.chars().any(char::is_control) {
        Err(FriendRequestDirectoryError)
    } else {
        Ok(())
    }
}

fn validate_text(value: &str) -> Result<(), FriendRequestDirectoryError> {
    if value.len() > MAX_TEXT_BYTES || value.chars().any(char::is_control) {
        Err(FriendRequestDirectoryError)
    } else {
        Ok(())
    }
}

#[derive(Clone, PartialEq, Message)]
struct FriendRequestQuery {
    #[prost(int32, tag = "1")]
    field1: i32,
    #[prost(int32, tag = "3")]
    field3: i32,
    #[prost(string, tag = "4")]
    self_uid: String,
    #[prost(int32, tag = "5")]
    field5: i32,
    #[prost(int32, tag = "6")]
    count: i32,
    #[prost(int32, tag = "8")]
    field8: i32,
    #[prost(int32, tag = "9")]
    field9: i32,
    #[prost(int32, tag = "12")]
    field12: i32,
    #[prost(int32, tag = "22")]
    field22: i32,
}

#[derive(Clone, PartialEq, Message)]
struct FriendRequestResponse {
    #[prost(message, optional, tag = "3")]
    info: Option<FriendRequestInfo>,
}

#[derive(Clone, PartialEq, Message)]
struct FriendRequestInfo {
    #[prost(message, repeated, tag = "7")]
    requests: Vec<FriendRequestWire>,
}

#[derive(Clone, PartialEq, Message)]
struct FriendRequestWire {
    #[prost(string, tag = "1")]
    target_uid: String,
    #[prost(string, tag = "2")]
    source_uid: String,
    #[prost(uint32, tag = "3")]
    state: u32,
    #[prost(uint32, tag = "4")]
    timestamp: u32,
    #[prost(string, tag = "5")]
    comment: String,
    #[prost(string, tag = "6")]
    source: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_query_retains_authenticated_account_uid() -> Result<(), Box<dyn std::error::Error>> {
        let encoded = encode_friend_request_list_request("u_self")?;
        let outer = qq_wire::decode_oidb_request(&encoded)?;
        let query = FriendRequestQuery::decode(outer.body())?;
        assert_eq!((outer.command(), outer.subcommand()), (0x05cf, 11));
        assert_eq!(query.self_uid, "u_self");
        assert_eq!(query.count, 80);
        Ok(())
    }

    #[test]
    fn response_preserves_pending_request_identity() -> Result<(), Box<dyn std::error::Error>> {
        let body = FriendRequestResponse {
            info: Some(FriendRequestInfo {
                requests: vec![FriendRequestWire {
                    target_uid: "u_self".to_owned(),
                    source_uid: "u_friend".to_owned(),
                    state: 1,
                    timestamp: 99,
                    comment: "hello".to_owned(),
                    source: "search".to_owned(),
                }],
            }),
        }
        .encode_to_vec();
        let response = qq_wire::encode_oidb_request(0x05cf, 11, &body, 0)?;
        let records = parse_friend_request_list(&response)?;
        assert_eq!(records.len(), 1);
        assert!(records[0].is_pending());
        assert_eq!(records[0].source_uid, "u_friend");
        Ok(())
    }
}
