use prost::Message;

use super::{ControlError, ControlRequest, request, validate_uid};

/// Encodes `set_friend_add_request` using an authenticated request UID.
///
/// # Errors
///
/// Returns an error for an unsafe Linux NT UID.
pub fn friend_request(source_uid: &str, approve: bool) -> Result<ControlRequest, ControlError> {
    validate_uid(source_uid)?;
    request(
        0x0b5d,
        44,
        "OidbSvcTrpcTcp.0xb5d_44",
        None,
        &FriendRequestDecision {
            decision: if approve { 3 } else { 5 },
            source_uid: source_uid.to_owned(),
        },
    )
}

#[derive(Clone, PartialEq, Message)]
struct FriendRequestDecision {
    #[prost(uint32, tag = "1")]
    decision: u32,
    #[prost(string, tag = "2")]
    source_uid: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decision_uses_evidence_backed_accept_and_reject_values()
    -> Result<(), Box<dyn std::error::Error>> {
        let accepted = friend_request("u_friend", true)?;
        let rejected = friend_request("u_friend", false)?;
        let accepted_outer = qq_wire::decode_oidb_request(accepted.body())?;
        let rejected_outer = qq_wire::decode_oidb_request(rejected.body())?;
        let accepted_body = FriendRequestDecision::decode(accepted_outer.body())?;
        let rejected_body = FriendRequestDecision::decode(rejected_outer.body())?;
        assert_eq!(
            (accepted_outer.command(), accepted_outer.subcommand()),
            (0x0b5d, 44)
        );
        assert_eq!(accepted.signing_operation(), None);
        assert_eq!(accepted_body.decision, 3);
        assert_eq!(rejected_body.decision, 5);
        assert_eq!(accepted_body.source_uid, "u_friend");
        Ok(())
    }
}
