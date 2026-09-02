use prost::Message;

use crate::{MessageClass, MessageDecodeError, MessageEnvelope};

const MAX_UID_BYTES: usize = 128;

/// Authenticated signal that QQ's group-request directory changed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GroupRequestSignal {
    /// A user asked to join a group.
    Join {
        /// Numeric QQ group identifier.
        group_id: u32,
        /// Current Linux NT UID of the applicant.
        target_uid: String,
    },
    /// A group member invited another user.
    Invitation {
        /// Numeric QQ group identifier.
        group_id: u32,
        /// Current Linux NT UID of the invited user.
        target_uid: String,
        /// Current Linux NT UID of the inviter.
        inviter_uid: String,
    },
    /// The current account was invited to a group.
    SelfInvitation {
        /// Numeric QQ group identifier.
        group_id: u32,
        /// Current Linux NT UID of the inviter.
        inviter_uid: String,
    },
}

/// Decodes an authenticated group-request signal.
///
/// The signal deliberately carries no sequence or comment; callers must recover those fields
/// from QQ's authenticated request directory before exposing an actionable event.
///
/// # Errors
///
/// Returns an error when a known request class carries malformed or unsafe data.
pub fn decode_group_request_signal(
    envelope: &MessageEnvelope,
) -> Result<Option<GroupRequestSignal>, MessageDecodeError> {
    let class = envelope.class();
    if !matches!(
        class,
        MessageClass::GroupJoinRequest
            | MessageClass::GroupInvitationRequest
            | MessageClass::GroupInvite
    ) {
        return Ok(None);
    }
    let content = envelope.payload().content().ok_or(MessageDecodeError)?;
    match class {
        MessageClass::GroupJoinRequest => decode_join(content).map(Some),
        MessageClass::GroupInvitationRequest => decode_invitation(content).map(Some),
        MessageClass::GroupInvite => decode_self_invitation(content).map(Some),
        _ => Ok(None),
    }
}

fn decode_join(input: &[u8]) -> Result<GroupRequestSignal, MessageDecodeError> {
    let value = JoinWire::decode(input).map_err(|_error| MessageDecodeError)?;
    validate(value.group_id, &value.target_uid)?;
    Ok(GroupRequestSignal::Join {
        group_id: value.group_id,
        target_uid: value.target_uid,
    })
}

fn decode_invitation(input: &[u8]) -> Result<GroupRequestSignal, MessageDecodeError> {
    let outer = InvitationOuterWire::decode(input).map_err(|_error| MessageDecodeError)?;
    if outer.command != 87 {
        return Err(MessageDecodeError);
    }
    let value = outer
        .info
        .and_then(|info| info.invitation)
        .ok_or(MessageDecodeError)?;
    validate(value.group_id, &value.target_uid)?;
    validate_uid(&value.inviter_uid)?;
    Ok(GroupRequestSignal::Invitation {
        group_id: value.group_id,
        target_uid: value.target_uid,
        inviter_uid: value.inviter_uid,
    })
}

fn decode_self_invitation(input: &[u8]) -> Result<GroupRequestSignal, MessageDecodeError> {
    let value = SelfInvitationWire::decode(input).map_err(|_error| MessageDecodeError)?;
    validate(value.group_id, &value.inviter_uid)?;
    Ok(GroupRequestSignal::SelfInvitation {
        group_id: value.group_id,
        inviter_uid: value.inviter_uid,
    })
}

fn validate(group_id: u32, uid: &str) -> Result<(), MessageDecodeError> {
    if group_id == 0 {
        return Err(MessageDecodeError);
    }
    validate_uid(uid)
}

fn validate_uid(uid: &str) -> Result<(), MessageDecodeError> {
    if uid.is_empty() || uid.len() > MAX_UID_BYTES || uid.chars().any(char::is_control) {
        Err(MessageDecodeError)
    } else {
        Ok(())
    }
}

#[derive(Clone, PartialEq, Message)]
struct JoinWire {
    #[prost(uint32, tag = "1")]
    group_id: u32,
    #[prost(string, tag = "3")]
    target_uid: String,
}

#[derive(Clone, PartialEq, Message)]
struct InvitationOuterWire {
    #[prost(uint32, tag = "1")]
    command: u32,
    #[prost(message, optional, tag = "2")]
    info: Option<InvitationInfoWire>,
}

#[derive(Clone, PartialEq, Message)]
struct InvitationInfoWire {
    #[prost(message, optional, tag = "1")]
    invitation: Option<InvitationWire>,
}

#[derive(Clone, PartialEq, Message)]
struct InvitationWire {
    #[prost(uint32, tag = "1")]
    group_id: u32,
    #[prost(string, tag = "5")]
    target_uid: String,
    #[prost(string, tag = "6")]
    inviter_uid: String,
}

#[derive(Clone, PartialEq, Message)]
struct SelfInvitationWire {
    #[prost(uint32, tag = "1")]
    group_id: u32,
    #[prost(string, tag = "5")]
    inviter_uid: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_signals_require_complete_identities() -> Result<(), Box<dyn std::error::Error>> {
        let join = decode_join(
            &JoinWire {
                group_id: 7,
                target_uid: "u_target".to_owned(),
            }
            .encode_to_vec(),
        )?;
        assert!(matches!(join, GroupRequestSignal::Join { group_id: 7, .. }));

        let invitation = decode_invitation(
            &InvitationOuterWire {
                command: 87,
                info: Some(InvitationInfoWire {
                    invitation: Some(InvitationWire {
                        group_id: 8,
                        target_uid: "u_target".to_owned(),
                        inviter_uid: "u_inviter".to_owned(),
                    }),
                }),
            }
            .encode_to_vec(),
        )?;
        assert!(matches!(
            invitation,
            GroupRequestSignal::Invitation { group_id: 8, .. }
        ));
        assert!(decode_join(&JoinWire::default().encode_to_vec()).is_err());
        Ok(())
    }
}
