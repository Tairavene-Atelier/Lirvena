//! Bounded QQ account and group control codecs for Lirvena.
#![forbid(unsafe_code)]

use prost::Message;

const MAX_UID_BYTES: usize = 128;
const MAX_TEXT_BYTES: usize = 4_096;

/// One encoded QQ control request and its transport metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ControlRequest {
    command: &'static str,
    body: Vec<u8>,
    signing_operation: Option<u32>,
}

impl ControlRequest {
    /// QQ command carried by the authenticated transport.
    #[must_use]
    pub const fn command(&self) -> &'static str {
        self.command
    }

    /// Exact protobuf body.
    #[must_use]
    pub fn body(&self) -> &[u8] {
        &self.body
    }

    /// Numeric Ceylith operation when this request requires a reserve.
    #[must_use]
    pub const fn signing_operation(&self) -> Option<u32> {
        self.signing_operation
    }
}

/// Opaque control codec failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ControlError;

impl core::fmt::Display for ControlError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("QQ control request or response is invalid")
    }
}

impl std::error::Error for ControlError {}

/// Encodes `set_group_kick`.
///
/// # Errors
///
/// Returns an error for invalid identifiers or excessive text.
pub fn group_kick(
    group_id: u32,
    uid: &str,
    reject_add_request: bool,
    reason: &str,
) -> Result<ControlRequest, ControlError> {
    validate_group_uid_text(group_id, uid, reason)?;
    request(
        0x08a0,
        1,
        "OidbSvcTrpcTcp.0x8a0_1",
        None,
        &KickBody {
            group_id,
            uid: uid.to_owned(),
            reject_add_request,
            reason: reason.to_owned(),
        },
    )
}

/// Encodes `set_group_ban`.
///
/// # Errors
///
/// Returns an error for invalid identifiers.
pub fn group_ban(
    group_id: u32,
    uid: &str,
    duration_seconds: u32,
) -> Result<ControlRequest, ControlError> {
    validate_group_uid_text(group_id, uid, "")?;
    request(
        0x1253,
        1,
        "OidbSvcTrpcTcp.0x1253_1",
        None,
        &MuteBody {
            group_id,
            kind: 1,
            value: Some(MuteValue {
                uid: uid.to_owned(),
                duration_seconds,
            }),
        },
    )
}

/// Encodes `set_group_whole_ban`.
///
/// # Errors
///
/// Returns an error for a zero group identifier.
pub fn group_whole_ban(group_id: u32, enabled: bool) -> Result<ControlRequest, ControlError> {
    validate_group(group_id)?;
    request(
        0x089a,
        0,
        "OidbSvcTrpcTcp.0x89a_0",
        Some(4),
        &WholeMuteBody {
            group_id,
            state: Some(WholeMuteState {
                value: Some(if enabled { u32::MAX } else { 0 }),
            }),
        },
    )
}

/// Encodes `set_group_admin`.
///
/// # Errors
///
/// Returns an error for invalid identifiers.
pub fn group_admin(
    group_id: u32,
    uid: &str,
    enabled: bool,
) -> Result<ControlRequest, ControlError> {
    validate_group_uid_text(group_id, uid, "")?;
    request(
        0x1096,
        1,
        "OidbSvcTrpcTcp.0x1096_1",
        None,
        &AdminBody {
            group_id,
            uid: uid.to_owned(),
            enabled,
        },
    )
}

/// Encodes `set_group_card`.
///
/// # Errors
///
/// Returns an error for invalid identifiers or excessive card text.
pub fn group_card(group_id: u32, uid: &str, card: &str) -> Result<ControlRequest, ControlError> {
    validate_group_uid_text(group_id, uid, card)?;
    member_text_request(group_id, uid, MemberTextBody::for_card(card))
}

/// Encodes `set_group_special_title`.
///
/// # Errors
///
/// Returns an error for invalid identifiers or excessive title text.
pub fn group_special_title(
    group_id: u32,
    uid: &str,
    title: &str,
) -> Result<ControlRequest, ControlError> {
    validate_group_uid_text(group_id, uid, title)?;
    member_text_request(group_id, uid, MemberTextBody::for_title(title))
}

/// Encodes `set_group_name`.
///
/// # Errors
///
/// Returns an error for a zero group identifier or excessive name text.
pub fn group_name(group_id: u32, name: &str) -> Result<ControlRequest, ControlError> {
    validate_group_uid_text(group_id, "valid", name)?;
    request(
        0x089a,
        15,
        "OidbSvcTrpcTcp.0x89a_15",
        Some(5),
        &GroupNameBody {
            group_id,
            value: Some(GroupNameValue {
                name: name.to_owned(),
            }),
        },
    )
}

/// Encodes `set_group_leave` for leaving a group without dissolving it.
///
/// # Errors
///
/// Returns an error for a zero group identifier.
pub fn group_leave(group_id: u32) -> Result<ControlRequest, ControlError> {
    validate_group(group_id)?;
    request(
        0x1097,
        1,
        "OidbSvcTrpcTcp.0x1097_1",
        None,
        &LeaveBody { group_id },
    )
}

/// Validates a generic OIDB action response.
///
/// # Errors
///
/// Returns an error for malformed data or a nonzero QQ result.
pub fn parse_control_response(input: &[u8]) -> Result<(), ControlError> {
    let response = OidbResponse::decode(input).map_err(|_error| ControlError)?;
    if response.error_code == 0 {
        Ok(())
    } else {
        Err(ControlError)
    }
}

fn member_text_request(
    group_id: u32,
    uid: &str,
    value: MemberTextBody,
) -> Result<ControlRequest, ControlError> {
    let (subcommand, command) = if value.card.is_some() {
        (3, "OidbSvcTrpcTcp.0x8fc_3")
    } else {
        (2, "OidbSvcTrpcTcp.0x8fc_2")
    };
    request(
        0x08fc,
        subcommand,
        command,
        None,
        &MemberTextRequest {
            group_id,
            value: Some(MemberTextBody {
                uid: uid.to_owned(),
                ..value
            }),
        },
    )
}

fn request(
    command: u32,
    subcommand: u32,
    route: &'static str,
    signing_operation: Option<u32>,
    body: &impl Message,
) -> Result<ControlRequest, ControlError> {
    let body = body.encode_to_vec();
    if body.is_empty() {
        return Err(ControlError);
    }
    Ok(ControlRequest {
        command: route,
        body: OidbRequest {
            command,
            subcommand,
            error_code: 0,
            body,
        }
        .encode_to_vec(),
        signing_operation,
    })
}

fn validate_group(group_id: u32) -> Result<(), ControlError> {
    if group_id == 0 {
        Err(ControlError)
    } else {
        Ok(())
    }
}

fn validate_group_uid_text(group_id: u32, uid: &str, text: &str) -> Result<(), ControlError> {
    validate_group(group_id)?;
    if uid.is_empty()
        || uid.len() > MAX_UID_BYTES
        || uid.chars().any(char::is_control)
        || text.len() > MAX_TEXT_BYTES
        || text.chars().any(char::is_control)
    {
        Err(ControlError)
    } else {
        Ok(())
    }
}

#[derive(Clone, PartialEq, Message)]
struct OidbRequest {
    #[prost(uint32, tag = "1")]
    command: u32,
    #[prost(uint32, tag = "2")]
    subcommand: u32,
    #[prost(uint32, tag = "3")]
    error_code: u32,
    #[prost(bytes = "vec", tag = "4")]
    body: Vec<u8>,
}

#[derive(Clone, PartialEq, Message)]
struct OidbResponse {
    #[prost(uint32, tag = "3")]
    error_code: u32,
}

#[derive(Clone, PartialEq, Message)]
struct KickBody {
    #[prost(uint32, tag = "1")]
    group_id: u32,
    #[prost(string, tag = "3")]
    uid: String,
    #[prost(bool, tag = "4")]
    reject_add_request: bool,
    #[prost(string, tag = "5")]
    reason: String,
}

#[derive(Clone, PartialEq, Message)]
struct MuteBody {
    #[prost(uint32, tag = "1")]
    group_id: u32,
    #[prost(uint32, tag = "2")]
    kind: u32,
    #[prost(message, optional, tag = "3")]
    value: Option<MuteValue>,
}

#[derive(Clone, PartialEq, Message)]
struct MuteValue {
    #[prost(string, tag = "1")]
    uid: String,
    #[prost(uint32, tag = "2")]
    duration_seconds: u32,
}

#[derive(Clone, PartialEq, Message)]
struct WholeMuteBody {
    #[prost(uint32, tag = "1")]
    group_id: u32,
    #[prost(message, optional, tag = "2")]
    state: Option<WholeMuteState>,
}

#[derive(Clone, PartialEq, Message)]
struct WholeMuteState {
    #[prost(uint32, optional, tag = "17")]
    value: Option<u32>,
}

#[derive(Clone, PartialEq, Message)]
struct AdminBody {
    #[prost(uint32, tag = "1")]
    group_id: u32,
    #[prost(string, tag = "2")]
    uid: String,
    #[prost(bool, tag = "3")]
    enabled: bool,
}

#[derive(Clone, PartialEq, Message)]
struct MemberTextRequest {
    #[prost(uint32, tag = "1")]
    group_id: u32,
    #[prost(message, optional, tag = "3")]
    value: Option<MemberTextBody>,
}

#[derive(Clone, PartialEq, Message)]
struct MemberTextBody {
    #[prost(string, tag = "1")]
    uid: String,
    #[prost(string, optional, tag = "5")]
    title: Option<String>,
    #[prost(int32, optional, tag = "6")]
    title_expires_at: Option<i32>,
    #[prost(string, optional, tag = "7")]
    title_copy: Option<String>,
    #[prost(string, optional, tag = "8")]
    card: Option<String>,
}

impl MemberTextBody {
    fn for_card(value: &str) -> Self {
        Self {
            uid: String::new(),
            title: None,
            title_expires_at: None,
            title_copy: None,
            card: Some(value.to_owned()),
        }
    }

    fn for_title(value: &str) -> Self {
        Self {
            uid: String::new(),
            title: Some(value.to_owned()),
            title_expires_at: Some(-1),
            title_copy: Some(value.to_owned()),
            card: None,
        }
    }
}

#[derive(Clone, PartialEq, Message)]
struct GroupNameBody {
    #[prost(uint32, tag = "1")]
    group_id: u32,
    #[prost(message, optional, tag = "2")]
    value: Option<GroupNameValue>,
}

#[derive(Clone, PartialEq, Message)]
struct GroupNameValue {
    #[prost(string, tag = "3")]
    name: String,
}

#[derive(Clone, Copy, PartialEq, Message)]
struct LeaveBody {
    #[prost(uint32, tag = "1")]
    group_id: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whole_unmute_forces_explicit_zero_and_sign_slot() -> Result<(), Box<dyn std::error::Error>> {
        let action = group_whole_ban(12_345, false)?;
        assert_eq!(action.signing_operation(), Some(4));
        let outer = OidbRequest::decode(action.body())?;
        let body = WholeMuteBody::decode(outer.body.as_slice())?;
        assert_eq!(body.state.and_then(|state| state.value), Some(0));
        Ok(())
    }

    #[test]
    fn member_card_and_title_use_distinct_subcommands() -> Result<(), Box<dyn std::error::Error>> {
        let card = OidbRequest::decode(group_card(1, "uid", "card")?.body())?;
        let title = OidbRequest::decode(group_special_title(1, "uid", "title")?.body())?;
        assert_eq!(card.subcommand, 3);
        assert_eq!(title.subcommand, 2);
        Ok(())
    }

    #[test]
    fn rejected_response_never_reports_success() {
        let rejected = OidbResponse { error_code: 1 }.encode_to_vec();
        assert_eq!(parse_control_response(&rejected), Err(ControlError));
    }
}
