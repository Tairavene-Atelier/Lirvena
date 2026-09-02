use prost::Message;

use crate::{ControlError, ControlRequest, request};

/// Encodes `set_essence_msg` from one retained group-message correlation.
///
/// # Errors
///
/// Returns an error when any correlation field is zero or exceeds the QQ wire width.
pub fn set_group_essence(
    group_code: u32,
    sequence: u64,
    random: u32,
) -> Result<ControlRequest, ControlError> {
    essence_request(group_code, sequence, random, 1, "OidbSvcTrpcTcp.0xeac_1")
}

/// Encodes `delete_essence_msg` from one retained group-message correlation.
///
/// # Errors
///
/// Returns an error when any correlation field is zero or exceeds the QQ wire width.
pub fn delete_group_essence(
    group_code: u32,
    sequence: u64,
    random: u32,
) -> Result<ControlRequest, ControlError> {
    essence_request(group_code, sequence, random, 2, "OidbSvcTrpcTcp.0xeac_2")
}

fn essence_request(
    group_code: u32,
    sequence: u64,
    random: u32,
    subcommand: u32,
    route: &'static str,
) -> Result<ControlRequest, ControlError> {
    let sequence = u32::try_from(sequence).map_err(|_error| ControlError)?;
    if group_code == 0 || sequence == 0 || random == 0 {
        return Err(ControlError);
    }
    request(
        0x0eac,
        subcommand,
        route,
        None,
        &EssenceBody {
            group_code,
            sequence,
            random,
        },
    )
}

#[derive(Clone, Copy, PartialEq, Message)]
struct EssenceBody {
    #[prost(uint32, tag = "1")]
    group_code: u32,
    #[prost(uint32, tag = "2")]
    sequence: u32,
    #[prost(uint32, tag = "3")]
    random: u32,
}

#[cfg(test)]
mod tests {
    use prost::Message;

    use super::*;

    #[test]
    fn set_and_delete_share_correlation_but_not_subcommand()
    -> Result<(), Box<dyn std::error::Error>> {
        let set = set_group_essence(100, 200, 300)?;
        let delete = delete_group_essence(100, 200, 300)?;
        let set_outer = qq_wire::decode_oidb_request(set.body())?;
        let delete_outer = qq_wire::decode_oidb_request(delete.body())?;
        assert_eq!((set_outer.command(), set_outer.subcommand()), (0x0eac, 1));
        assert_eq!(
            (delete_outer.command(), delete_outer.subcommand()),
            (0x0eac, 2)
        );
        assert_eq!(set_outer.body(), delete_outer.body());
        assert_eq!(
            EssenceBody::decode(set_outer.body())?,
            EssenceBody {
                group_code: 100,
                sequence: 200,
                random: 300,
            }
        );
        Ok(())
    }

    #[test]
    fn missing_or_out_of_width_correlation_is_rejected() {
        assert!(set_group_essence(1, u64::from(u32::MAX) + 1, 1).is_err());
        assert!(set_group_essence(1, 1, 0).is_err());
        assert!(delete_group_essence(0, 1, 1).is_err());
    }
}
