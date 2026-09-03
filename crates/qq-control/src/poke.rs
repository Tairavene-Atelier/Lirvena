use prost::Message;

use crate::{ControlError, ControlRequest, request};

/// Encodes the dedicated QQ poke operation for a friend or group conversation.
///
/// # Errors
///
/// Returns an error when the peer or target numeric identity is zero.
pub fn poke(group: bool, peer_uin: u32, target_uin: u32) -> Result<ControlRequest, ControlError> {
    if peer_uin == 0 || target_uin == 0 {
        return Err(ControlError);
    }
    request(
        0x0ed3,
        1,
        "OidbSvcTrpcTcp.0xed3_1",
        None,
        &PokeBody {
            target_uin,
            group_uin: if group { peer_uin } else { 0 },
            friend_uin: if group { 0 } else { peer_uin },
            extension: Some(0),
        },
    )
}

#[derive(Clone, Copy, PartialEq, Message)]
struct PokeBody {
    #[prost(uint32, tag = "1")]
    target_uin: u32,
    #[prost(uint32, tag = "2")]
    group_uin: u32,
    #[prost(uint32, tag = "5")]
    friend_uin: u32,
    #[prost(uint32, optional, tag = "6")]
    extension: Option<u32>,
}

#[cfg(test)]
mod tests {
    use prost::Message;

    use super::{PokeBody, poke};

    #[test]
    fn friend_and_group_use_disjoint_peer_fields() -> Result<(), Box<dyn std::error::Error>> {
        let friend = poke(false, 42, 43)?;
        assert_eq!(friend.signing_operation(), None);
        let outer = qq_wire::decode_oidb_request(friend.body())?;
        assert_eq!((outer.command(), outer.subcommand()), (0x0ed3, 1));
        let body = PokeBody::decode(outer.body())?;
        assert_eq!(
            (
                body.target_uin,
                body.group_uin,
                body.friend_uin,
                body.extension
            ),
            (43, 0, 42, Some(0))
        );

        let group = poke(true, 44, 45)?;
        let outer = qq_wire::decode_oidb_request(group.body())?;
        let body = PokeBody::decode(outer.body())?;
        assert_eq!(
            (body.target_uin, body.group_uin, body.friend_uin),
            (45, 44, 0)
        );
        assert!(poke(false, 0, 1).is_err());
        assert!(poke(true, 1, 0).is_err());
        Ok(())
    }
}
