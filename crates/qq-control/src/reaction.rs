use prost::Message;

use crate::{ControlError, ControlRequest, request_reserved};

/// Authenticated conversation selected for the legacy message reaction route.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EmojiChainTarget<'a> {
    /// One group conversation.
    Group(u32),
    /// One direct conversation using the current peer UID.
    Private(&'a str),
}

/// Encodes an add or remove operation for one group-message reaction.
///
/// # Errors
///
/// Returns an error for zero identifiers, an out-of-width sequence, or a reaction code that is
/// not a non-zero decimal `u32` value.
pub fn group_reaction(
    group_code: u32,
    sequence: u64,
    code: &str,
    add: bool,
) -> Result<ControlRequest, ControlError> {
    let sequence = u32::try_from(sequence).map_err(|_error| ControlError)?;
    if group_code == 0
        || sequence == 0
        || code.is_empty()
        || code.len() > 10
        || code.bytes().any(|byte| !byte.is_ascii_digit())
        || code.parse::<u32>().ok().is_none_or(|value| value == 0)
    {
        return Err(ControlError);
    }
    let subcommand = if add { 1 } else { 2 };
    let route = if add {
        "OidbSvcTrpcTcp.0x9082_1"
    } else {
        "OidbSvcTrpcTcp.0x9082_2"
    };
    request_reserved(
        0x9082,
        subcommand,
        route,
        None,
        1,
        &ReactionBody {
            group_code,
            sequence,
            code: code.to_owned(),
            kind: if code.len() > 3 { 2 } else { 1 },
            field_six: false,
            field_seven: false,
        },
    )
}

/// Encodes the frozen legacy add-only message reaction request.
///
/// # Errors
///
/// Returns an error for an incomplete target, a zero face, or an out-of-width message sequence.
pub fn join_emoji_chain(
    target: EmojiChainTarget<'_>,
    sequence: u64,
    face_id: u32,
) -> Result<ControlRequest, ControlError> {
    let sequence = u32::try_from(sequence).map_err(|_error| ControlError)?;
    if sequence == 0 || face_id == 0 {
        return Err(ControlError);
    }
    let (kind, group_code, uid) = match target {
        EmojiChainTarget::Group(group_code) if group_code != 0 => (2, Some(group_code), None),
        EmojiChainTarget::Private(uid)
            if !uid.is_empty()
                && uid.len() <= crate::MAX_UID_BYTES
                && !uid.chars().any(char::is_control) =>
        {
            (1, None, Some(uid.to_owned()))
        }
        EmojiChainTarget::Group(_) | EmojiChainTarget::Private(_) => return Err(ControlError),
    };
    crate::request(
        0x90ee,
        1,
        "OidbSvcTrpcTcp.0x90ee_1",
        None,
        &EmojiChainBody {
            face_id,
            sequence,
            repeated_sequence: sequence,
            kind,
            group_code,
            uid,
        },
    )
}

#[derive(Clone, PartialEq, Message)]
struct ReactionBody {
    #[prost(uint32, tag = "2")]
    group_code: u32,
    #[prost(uint32, tag = "3")]
    sequence: u32,
    #[prost(string, tag = "4")]
    code: String,
    #[prost(uint32, tag = "5")]
    kind: u32,
    #[prost(bool, tag = "6")]
    field_six: bool,
    #[prost(bool, tag = "7")]
    field_seven: bool,
}

#[derive(Clone, PartialEq, Message)]
struct EmojiChainBody {
    #[prost(uint32, tag = "1")]
    face_id: u32,
    #[prost(uint32, tag = "2")]
    sequence: u32,
    #[prost(uint32, tag = "3")]
    repeated_sequence: u32,
    #[prost(int32, tag = "4")]
    kind: i32,
    #[prost(uint32, optional, tag = "5")]
    group_code: Option<u32>,
    #[prost(string, optional, tag = "6")]
    uid: Option<String>,
}

#[cfg(test)]
mod tests {
    use prost::Message;

    use super::{EmojiChainBody, EmojiChainTarget, ReactionBody, group_reaction, join_emoji_chain};

    #[test]
    fn legacy_group_and_private_targets_are_disjoint() -> Result<(), Box<dyn std::error::Error>> {
        let group = join_emoji_chain(EmojiChainTarget::Group(42), 43, 44)?;
        let outer = qq_wire::decode_oidb_request(group.body())?;
        assert_eq!((outer.command(), outer.subcommand()), (0x90ee, 1));
        assert_eq!(
            EmojiChainBody::decode(outer.body())?,
            EmojiChainBody {
                face_id: 44,
                sequence: 43,
                repeated_sequence: 43,
                kind: 2,
                group_code: Some(42),
                uid: None,
            }
        );

        let private = join_emoji_chain(EmojiChainTarget::Private("u_peer"), 43, 44)?;
        let outer = qq_wire::decode_oidb_request(private.body())?;
        assert_eq!(
            EmojiChainBody::decode(outer.body())?,
            EmojiChainBody {
                face_id: 44,
                sequence: 43,
                repeated_sequence: 43,
                kind: 1,
                group_code: None,
                uid: Some("u_peer".to_owned()),
            }
        );
        Ok(())
    }

    #[test]
    fn legacy_reaction_rejects_incomplete_correlations() {
        assert!(join_emoji_chain(EmojiChainTarget::Group(0), 1, 1).is_err());
        assert!(join_emoji_chain(EmojiChainTarget::Private(""), 1, 1).is_err());
        assert!(join_emoji_chain(EmojiChainTarget::Group(1), 0, 1).is_err());
        assert!(join_emoji_chain(EmojiChainTarget::Group(1), 1, 0).is_err());
    }

    #[test]
    fn add_and_remove_preserve_uid_outer_flag_and_reaction_kind()
    -> Result<(), Box<dyn std::error::Error>> {
        let add = group_reaction(42, 43, "14", true)?;
        let outer = qq_wire::decode_oidb_request(add.body())?;
        assert_eq!(
            (outer.command(), outer.subcommand(), outer.reserved()),
            (0x9082, 1, 1)
        );
        assert_eq!(add.signing_operation(), None);
        assert_eq!(
            ReactionBody::decode(outer.body())?,
            ReactionBody {
                group_code: 42,
                sequence: 43,
                code: "14".to_owned(),
                kind: 1,
                field_six: false,
                field_seven: false,
            }
        );

        let remove = group_reaction(42, 43, "10001", false)?;
        let outer = qq_wire::decode_oidb_request(remove.body())?;
        assert_eq!((outer.subcommand(), outer.reserved()), (2, 1));
        assert_eq!(ReactionBody::decode(outer.body())?.kind, 2);
        Ok(())
    }

    #[test]
    fn invalid_correlations_and_codes_fail_closed() {
        assert!(group_reaction(0, 1, "14", true).is_err());
        assert!(group_reaction(1, 0, "14", true).is_err());
        assert!(group_reaction(1, u64::from(u32::MAX) + 1, "14", true).is_err());
        assert!(group_reaction(1, 1, "0", true).is_err());
        assert!(group_reaction(1, 1, "emoji", true).is_err());
    }
}
