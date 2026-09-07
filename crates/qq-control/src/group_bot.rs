use prost::Message;

use super::{ControlError, ControlRequest, request, validate_group_uid_text};

const MAX_CALLBACK_BYTES: usize = 4096;

/// Enables or disables a group bot through the audited 52194 OIDB route.
///
/// # Errors
///
/// Returns an error for zero identifiers or an invalid state value.
pub fn group_bot_status(
    group_id: u32,
    bot_id: u32,
    state: u32,
) -> Result<ControlRequest, ControlError> {
    if bot_id == 0 || state > 1 {
        return Err(ControlError);
    }
    validate_group_uid_text(group_id, "bot", "")?;
    request(
        0x907d,
        1,
        "OidbSvcTrpcTcp.0x907d_1",
        None,
        &GroupBotStatus {
            bot_id,
            kind: 2,
            state,
            group_id,
        },
    )
}

/// Sends the bounded callback pair expected by a group bot interaction.
///
/// # Errors
///
/// Returns an error for zero identifiers, control characters, or oversized callback data.
pub fn group_bot_callback(
    group_id: u32,
    bot_id: u32,
    callback_id: &str,
    callback_data: &str,
) -> Result<ControlRequest, ControlError> {
    if bot_id == 0
        || invalid_callback(callback_id)
        || invalid_callback(callback_data)
        || group_id == 0
    {
        return Err(ControlError);
    }
    request(
        0x112e,
        1,
        "OidbSvcTrpcTcp.0x112e_1",
        Some(12),
        &GroupBotCallback {
            bot_id,
            sequence: 11_111,
            callback_id: callback_id.to_owned(),
            callback_data: callback_data.to_owned(),
            reserved: 0,
            group_id,
            group_kind: 1,
        },
    )
}

fn invalid_callback(value: &str) -> bool {
    value.len() > MAX_CALLBACK_BYTES || value.chars().any(char::is_control)
}

#[derive(Clone, Copy, PartialEq, Message)]
struct GroupBotStatus {
    #[prost(uint32, tag = "1")]
    bot_id: u32,
    #[prost(uint32, tag = "2")]
    kind: u32,
    #[prost(uint32, tag = "3")]
    state: u32,
    #[prost(uint32, tag = "4")]
    group_id: u32,
}

#[derive(Clone, PartialEq, Message)]
struct GroupBotCallback {
    #[prost(uint32, tag = "3")]
    bot_id: u32,
    #[prost(uint32, tag = "4")]
    sequence: u32,
    #[prost(string, tag = "5")]
    callback_id: String,
    #[prost(string, tag = "6")]
    callback_data: String,
    #[prost(uint32, tag = "7")]
    reserved: u32,
    #[prost(uint32, tag = "8")]
    group_id: u32,
    #[prost(uint32, tag = "9")]
    group_kind: u32,
}

#[cfg(test)]
mod tests {
    use prost::Message;

    use super::{GroupBotCallback, GroupBotStatus, group_bot_callback, group_bot_status};

    #[test]
    fn bot_status_matches_frozen_oidb_shape() -> Result<(), Box<dyn std::error::Error>> {
        let request = group_bot_status(123, 456, 1)?;
        let outer = qq_wire::decode_oidb_request(request.body())?;
        let body = GroupBotStatus::decode(outer.body())?;
        assert_eq!((outer.command(), outer.subcommand()), (0x907d, 1));
        assert_eq!(
            (body.bot_id, body.kind, body.state, body.group_id),
            (456, 2, 1, 123)
        );
        assert!(group_bot_status(123, 456, 2).is_err());
        Ok(())
    }

    #[test]
    fn bot_callback_is_bounded_and_uses_fixed_route_fields()
    -> Result<(), Box<dyn std::error::Error>> {
        let request = group_bot_callback(123, 456, "button", "payload")?;
        let outer = qq_wire::decode_oidb_request(request.body())?;
        let body = GroupBotCallback::decode(outer.body())?;
        assert_eq!((outer.command(), outer.subcommand()), (0x112e, 1));
        assert_eq!(request.signing_operation(), Some(12));
        assert_eq!(
            (body.sequence, body.reserved, body.group_kind),
            (11_111, 0, 1)
        );
        assert_eq!(
            (body.callback_id.as_str(), body.callback_data.as_str()),
            ("button", "payload")
        );
        assert!(group_bot_callback(123, 456, "bad\n", "payload").is_err());
        Ok(())
    }
}
