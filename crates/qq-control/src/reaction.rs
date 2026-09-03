use prost::Message;

use crate::{ControlError, ControlRequest, request_reserved};

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

#[cfg(test)]
mod tests {
    use prost::Message;

    use super::{ReactionBody, group_reaction};

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
