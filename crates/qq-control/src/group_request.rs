use prost::Message;

use super::{ControlError, ControlRequest, request_reserved, validate_group_uid_text};

/// Encodes `set_group_add_request` using the authenticated request reference.
///
/// # Errors
///
/// Returns an error for a missing request identity or excessive rejection reason.
pub fn group_request(
    sequence: u64,
    event_type: u32,
    group_id: u32,
    approve: bool,
    reason: &str,
) -> Result<ControlRequest, ControlError> {
    validate_group_uid_text(group_id, "valid", reason)?;
    if sequence == 0 || !matches!(event_type, 1 | 2 | 22) {
        return Err(ControlError);
    }
    request_reserved(
        0x10c8,
        1,
        "OidbSvcTrpcTcp.0x10c8_1",
        Some(6),
        1,
        &GroupRequestDecision {
            decision: if approve { 1 } else { 2 },
            request: Some(GroupRequestReference {
                sequence,
                event_type,
                group_id,
                message: reason.to_owned(),
            }),
        },
    )
}

#[derive(Clone, PartialEq, Message)]
struct GroupRequestDecision {
    #[prost(uint32, tag = "1")]
    decision: u32,
    #[prost(message, optional, tag = "2")]
    request: Option<GroupRequestReference>,
}

#[derive(Clone, PartialEq, Message)]
struct GroupRequestReference {
    #[prost(uint64, tag = "1")]
    sequence: u64,
    #[prost(uint32, tag = "2")]
    event_type: u32,
    #[prost(uint32, tag = "3")]
    group_id: u32,
    #[prost(string, tag = "4")]
    message: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decision_retains_authenticated_reference() -> Result<(), Box<dyn std::error::Error>> {
        let request = group_request(77, 22, 12_345, false, "reason")?;
        let outer = qq_wire::decode_oidb_request(request.body())?;
        let decision = GroupRequestDecision::decode(outer.body())?;
        let reference = decision.request.ok_or("missing reference")?;
        assert_eq!((outer.command(), outer.subcommand()), (0x10c8, 1));
        assert_eq!(outer.reserved(), 1);
        assert_eq!(request.signing_operation(), Some(6));
        assert_eq!(decision.decision, 2);
        assert_eq!(
            (reference.sequence, reference.event_type, reference.group_id),
            (77, 22, 12_345)
        );
        assert_eq!(reference.message, "reason");
        Ok(())
    }
}
