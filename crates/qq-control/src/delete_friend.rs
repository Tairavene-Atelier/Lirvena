use prost::Message;

use crate::{ControlError, ControlRequest, request, validate_uid};

/// Encodes the frozen Linux NT friend-deletion request.
///
/// # Errors
///
/// Returns an error for an empty, excessive, or control-bearing peer UID.
pub fn delete_friend(uid: &str, block: bool) -> Result<ControlRequest, ControlError> {
    validate_uid(uid)?;
    request(
        0x126b,
        0,
        "OidbSvcTrpcTcp.0x126b_0",
        None,
        &DeleteFriendRequest {
            target: Some(DeleteFriendTarget {
                uid: uid.to_owned(),
                constants: Some(DeleteFriendConstants {
                    first: 130,
                    second: 109,
                    nested: Some(DeleteFriendNestedConstants {
                        first: 8,
                        second: 8,
                        third: 50,
                    }),
                }),
                block,
                field_four: false,
            }),
        },
    )
}

#[derive(Clone, PartialEq, Message)]
struct DeleteFriendRequest {
    #[prost(message, optional, tag = "1")]
    target: Option<DeleteFriendTarget>,
}

#[derive(Clone, PartialEq, Message)]
struct DeleteFriendTarget {
    #[prost(string, tag = "1")]
    uid: String,
    #[prost(message, optional, tag = "2")]
    constants: Option<DeleteFriendConstants>,
    #[prost(bool, tag = "3")]
    block: bool,
    #[prost(bool, tag = "4")]
    field_four: bool,
}

#[derive(Clone, Copy, PartialEq, Message)]
struct DeleteFriendConstants {
    #[prost(uint32, tag = "1")]
    first: u32,
    #[prost(uint32, tag = "2")]
    second: u32,
    #[prost(message, optional, tag = "3")]
    nested: Option<DeleteFriendNestedConstants>,
}

#[derive(Clone, Copy, PartialEq, Message)]
struct DeleteFriendNestedConstants {
    #[prost(uint32, tag = "1")]
    first: u32,
    #[prost(uint32, tag = "2")]
    second: u32,
    #[prost(uint32, tag = "3")]
    third: u32,
}

#[cfg(test)]
mod tests {
    use prost::Message;

    use super::{DeleteFriendRequest, delete_friend};

    #[test]
    fn request_matches_frozen_nested_shape() -> Result<(), Box<dyn std::error::Error>> {
        let request = delete_friend("u_peer", true)?;
        assert_eq!(request.signing_operation(), None);
        let outer = qq_wire::decode_oidb_request(request.body())?;
        assert_eq!((outer.command(), outer.subcommand()), (0x126b, 0));
        let body = DeleteFriendRequest::decode(outer.body())?;
        let target = body.target.ok_or("target missing")?;
        let constants = target.constants.ok_or("constants missing")?;
        let nested = constants.nested.ok_or("nested constants missing")?;
        assert_eq!(target.uid, "u_peer");
        assert!(target.block);
        assert!(!target.field_four);
        assert_eq!((constants.first, constants.second), (130, 109));
        assert_eq!((nested.first, nested.second, nested.third), (8, 8, 50));
        Ok(())
    }

    #[test]
    fn invalid_uid_fails_closed() {
        assert!(delete_friend("", false).is_err());
        assert!(delete_friend("u\npeer", false).is_err());
    }
}
