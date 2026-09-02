use prost::Message;

use crate::{MessageClass, MessageDecodeError, MessageEnvelope};

const MAX_UID_BYTES: usize = 128;

/// Authenticated group-system notice decoded from a QQ message envelope.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GroupNotice {
    /// One member gained or lost administrator status.
    Administrator {
        /// Numeric QQ group identifier.
        group_id: u32,
        /// Current Linux NT member UID.
        member_uid: String,
        /// Whether administrator status was granted.
        enabled: bool,
    },
    /// One member entered a group.
    MemberIncrease {
        /// Numeric QQ group identifier.
        group_id: u32,
        /// Current Linux NT member UID.
        member_uid: String,
        /// Inviter UID when QQ supplied it.
        operator_uid: Option<String>,
        /// Evidence-backed increase subtype.
        kind: MemberIncreaseKind,
    },
    /// One member left or was removed from a group.
    MemberDecrease {
        /// Numeric QQ group identifier.
        group_id: u32,
        /// Current Linux NT member UID.
        member_uid: String,
        /// Operator UID when QQ supplied it.
        operator_uid: Option<String>,
        /// Evidence-backed decrease subtype.
        kind: MemberDecreaseKind,
    },
}

/// QQ group-member increase subtype.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemberIncreaseKind {
    /// A join request was approved.
    Approve,
    /// An existing member invited the new member.
    Invite,
    /// Authenticated subtype not yet assigned a public semantic mapping.
    Unknown(u32),
}

/// QQ group-member decrease subtype.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemberDecreaseKind {
    /// The current Lirvena account was removed.
    KickMe,
    /// The group was disbanded.
    Disband,
    /// The member left voluntarily.
    Leave,
    /// An operator removed the member.
    Kick,
    /// Authenticated subtype not yet assigned a public semantic mapping.
    Unknown(u32),
}

/// Decodes a compiled group-system notice from one authenticated envelope.
///
/// # Errors
///
/// Returns an error when a known notice class carries malformed, incomplete, or unsafe data.
pub fn decode_group_notice(
    envelope: &MessageEnvelope,
) -> Result<Option<GroupNotice>, MessageDecodeError> {
    let Some(content) = envelope.payload().content() else {
        return match envelope.class() {
            MessageClass::GroupAdministratorChange
            | MessageClass::GroupMemberIncrease
            | MessageClass::GroupMemberDecrease => Err(MessageDecodeError),
            _ => Ok(None),
        };
    };
    match envelope.class() {
        MessageClass::GroupAdministratorChange => decode_administrator(content).map(Some),
        MessageClass::GroupMemberIncrease => decode_increase(content).map(Some),
        MessageClass::GroupMemberDecrease => decode_decrease(content).map(Some),
        _ => Ok(None),
    }
}

fn decode_administrator(input: &[u8]) -> Result<GroupNotice, MessageDecodeError> {
    let notice = AdministratorWire::decode(input).map_err(|_error| MessageDecodeError)?;
    let body = notice.body.ok_or(MessageDecodeError)?;
    let (member_uid, enabled) = match (body.enable, body.disable) {
        (Some(value), None) => (value.member_uid, true),
        (None, Some(value)) => (value.member_uid, false),
        _ => return Err(MessageDecodeError),
    };
    validate_identity(notice.group_id, &member_uid)?;
    Ok(GroupNotice::Administrator {
        group_id: notice.group_id,
        member_uid,
        enabled,
    })
}

fn decode_increase(input: &[u8]) -> Result<GroupNotice, MessageDecodeError> {
    let notice = MemberChangeWire::decode(input).map_err(|_error| MessageDecodeError)?;
    validate_identity(notice.group_id, &notice.member_uid)?;
    let kind = match nonzero(notice.primary_type, notice.secondary_type) {
        130 => MemberIncreaseKind::Approve,
        131 => MemberIncreaseKind::Invite,
        value => MemberIncreaseKind::Unknown(value),
    };
    let operator_uid = optional_plain_uid(notice.operator)?;
    Ok(GroupNotice::MemberIncrease {
        group_id: notice.group_id,
        member_uid: notice.member_uid,
        operator_uid,
        kind,
    })
}

fn decode_decrease(input: &[u8]) -> Result<GroupNotice, MessageDecodeError> {
    let notice = MemberChangeWire::decode(input).map_err(|_error| MessageDecodeError)?;
    validate_identity(notice.group_id, &notice.member_uid)?;
    let (kind, operator_uid) = match notice.primary_type {
        3 => (
            MemberDecreaseKind::KickMe,
            optional_nested_uid(notice.operator)?,
        ),
        129 => (
            MemberDecreaseKind::Disband,
            optional_plain_uid(notice.operator)?,
        ),
        130 => (
            MemberDecreaseKind::Leave,
            optional_plain_uid(notice.operator)?,
        ),
        131 => (
            MemberDecreaseKind::Kick,
            optional_plain_uid(notice.operator)?,
        ),
        value => (
            MemberDecreaseKind::Unknown(value),
            optional_plain_uid(notice.operator)?,
        ),
    };
    Ok(GroupNotice::MemberDecrease {
        group_id: notice.group_id,
        member_uid: notice.member_uid,
        operator_uid,
        kind,
    })
}

fn optional_plain_uid(input: Option<Vec<u8>>) -> Result<Option<String>, MessageDecodeError> {
    input
        .filter(|value| !value.is_empty())
        .map(|value| String::from_utf8(value).map_err(|_error| MessageDecodeError))
        .transpose()?
        .map(validate_uid_owned)
        .transpose()
}

fn optional_nested_uid(input: Option<Vec<u8>>) -> Result<Option<String>, MessageDecodeError> {
    let Some(input) = input.filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    let value = OperatorWire::decode(input.as_slice())
        .map_err(|_error| MessageDecodeError)?
        .identity
        .and_then(|identity| identity.uid)
        .ok_or(MessageDecodeError)?;
    validate_uid_owned(value).map(Some)
}

fn validate_identity(group_id: u32, uid: &str) -> Result<(), MessageDecodeError> {
    if group_id == 0 {
        return Err(MessageDecodeError);
    }
    validate_uid(uid)
}

fn validate_uid_owned(uid: String) -> Result<String, MessageDecodeError> {
    validate_uid(&uid)?;
    Ok(uid)
}

fn validate_uid(uid: &str) -> Result<(), MessageDecodeError> {
    if uid.is_empty() || uid.len() > MAX_UID_BYTES || uid.chars().any(char::is_control) {
        Err(MessageDecodeError)
    } else {
        Ok(())
    }
}

const fn nonzero(first: u32, second: u32) -> u32 {
    if first == 0 { second } else { first }
}

#[derive(Clone, PartialEq, Message)]
struct AdministratorWire {
    #[prost(uint32, tag = "1")]
    group_id: u32,
    #[prost(message, optional, tag = "4")]
    body: Option<AdministratorBodyWire>,
}

#[derive(Clone, PartialEq, Message)]
struct AdministratorBodyWire {
    #[prost(message, optional, tag = "1")]
    disable: Option<AdministratorMemberWire>,
    #[prost(message, optional, tag = "2")]
    enable: Option<AdministratorMemberWire>,
}

#[derive(Clone, PartialEq, Message)]
struct AdministratorMemberWire {
    #[prost(string, tag = "1")]
    member_uid: String,
}

#[derive(Clone, PartialEq, Message)]
struct MemberChangeWire {
    #[prost(uint32, tag = "1")]
    group_id: u32,
    #[prost(string, tag = "3")]
    member_uid: String,
    #[prost(uint32, tag = "4")]
    primary_type: u32,
    #[prost(bytes = "vec", optional, tag = "5")]
    operator: Option<Vec<u8>>,
    #[prost(uint32, tag = "6")]
    secondary_type: u32,
}

#[derive(Clone, PartialEq, Message)]
struct OperatorWire {
    #[prost(message, optional, tag = "1")]
    identity: Option<OperatorIdentityWire>,
}

#[derive(Clone, PartialEq, Message)]
struct OperatorIdentityWire {
    #[prost(string, optional, tag = "1")]
    uid: Option<String>,
}
