use prost::Message;

use crate::MessageDecodeError;

use super::{
    ContentHead, MessageBody, MessageControl, MessageWire, RichText, RoutingHead, TransferRoute,
    parse_send_message_response, valid_uid,
};

const MAX_FILE_TEXT_BYTES: usize = 4 * 1024;

/// Bounded input for one uploaded private-file message.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrivateFileMessageInput<'a> {
    /// Current recipient UID resolved from the friend directory.
    pub recipient_uid: &'a str,
    /// QQ file identifier returned by upload negotiation.
    pub file_id: &'a str,
    /// QQ file hash/addon returned by upload negotiation.
    pub file_hash: &'a str,
    /// User-visible file name.
    pub file_name: &'a str,
    /// Complete object size.
    pub file_size: u64,
    /// MD5 of the complete object.
    pub md5: &'a [u8; 16],
    /// Seven-day expiration timestamp.
    pub expire_at: u32,
    /// Non-zero local client sequence.
    pub client_sequence: u32,
    /// Non-zero local message random.
    pub random: u32,
    /// Current Unix timestamp used by the message control field.
    pub unix_seconds: u32,
}

/// Encodes the frozen Linux private-file final send message.
///
/// # Errors
///
/// Returns an error for invalid identifiers, metadata, timestamps, or correlations.
pub fn encode_private_file_message(
    input: &PrivateFileMessageInput<'_>,
) -> Result<Vec<u8>, MessageDecodeError> {
    if !valid_uid(input.recipient_uid)
        || !valid_file_text(input.file_id)
        || !valid_file_text(input.file_hash)
        || !valid_file_text(input.file_name)
        || input.file_size == 0
        || input.expire_at <= input.unix_seconds
        || input.client_sequence == 0
        || input.random == 0
    {
        return Err(MessageDecodeError);
    }
    let control_time = i32::try_from(input.unix_seconds).map_err(|_error| MessageDecodeError)?;
    let expire_at = i32::try_from(input.expire_at).map_err(|_error| MessageDecodeError)?;
    let content = PrivateFileContent {
        file: Some(PrivateFileInfo {
            file_type: Some(0),
            file_id: Some(input.file_id.to_owned()),
            md5: Some(input.md5.to_vec()),
            file_name: Some(input.file_name.to_owned()),
            file_size: Some(input.file_size),
            subcommand: Some(1),
            danger_level: Some(0),
            expire_at: Some(expire_at),
            file_hash: Some(input.file_hash.to_owned()),
        }),
    }
    .encode_to_vec();
    Ok(MessageWire {
        routing: Some(RoutingHead {
            c2c: None,
            group: None,
            transfer: Some(TransferRoute {
                command: Some(4),
                uid: Some(input.recipient_uid.to_owned()),
            }),
        }),
        content: Some(ContentHead {
            message_type: 1,
            subtype: Some(0),
            c2c_command: Some(0),
        }),
        body: Some(MessageBody {
            rich_text: Some(RichText {
                elements: Vec::new(),
            }),
            content: Some(content),
        }),
        client_sequence: Some(input.client_sequence),
        random: Some(input.random),
        control: Some(MessageControl {
            message_flag: control_time,
        }),
    }
    .encode_to_vec())
}

/// Validates QQ's final acknowledgement for a private-file message.
///
/// # Errors
///
/// Returns an error for malformed, rejected, or uncorrelated acknowledgements.
pub fn validate_private_file_message_response(input: &[u8]) -> Result<(), MessageDecodeError> {
    let response = parse_send_message_response(input)?;
    if response.result == 0 {
        Ok(())
    } else {
        Err(MessageDecodeError)
    }
}

#[derive(Clone, PartialEq, Message)]
struct PrivateFileContent {
    #[prost(message, optional, tag = "1")]
    file: Option<PrivateFileInfo>,
}

#[derive(Clone, PartialEq, Message)]
struct PrivateFileInfo {
    #[prost(int32, optional, tag = "1")]
    file_type: Option<i32>,
    #[prost(string, optional, tag = "3")]
    file_id: Option<String>,
    #[prost(bytes = "vec", optional, tag = "4")]
    md5: Option<Vec<u8>>,
    #[prost(string, optional, tag = "5")]
    file_name: Option<String>,
    #[prost(uint64, optional, tag = "6")]
    file_size: Option<u64>,
    #[prost(int32, optional, tag = "9")]
    subcommand: Option<i32>,
    #[prost(int32, optional, tag = "50")]
    danger_level: Option<i32>,
    #[prost(int32, optional, tag = "55")]
    expire_at: Option<i32>,
    #[prost(string, optional, tag = "57")]
    file_hash: Option<String>,
}

fn valid_file_text(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_FILE_TEXT_BYTES
        && value.trim() == value
        && !value.chars().any(char::is_control)
}

#[cfg(test)]
mod tests {
    use prost::Message;

    use super::{
        MessageWire, PrivateFileContent, PrivateFileMessageInput, encode_private_file_message,
        validate_private_file_message_response,
    };
    use crate::outbound::SendResponse;

    #[test]
    fn final_message_preserves_transfer_route_and_file_content()
    -> Result<(), Box<dyn std::error::Error>> {
        let body = encode_private_file_message(&PrivateFileMessageInput {
            recipient_uid: "u_peer",
            file_id: "file-id",
            file_hash: "addon",
            file_name: "a.bin",
            file_size: 3,
            md5: &[7; 16],
            expire_at: 700_000,
            client_sequence: 8,
            random: 9,
            unix_seconds: 100,
        })?;
        let message = MessageWire::decode(body.as_slice())?;
        let route = message
            .routing
            .and_then(|routing| routing.transfer)
            .ok_or("transfer route missing")?;
        assert_eq!(
            (route.command, route.uid.as_deref()),
            (Some(4), Some("u_peer"))
        );
        let message_body = message.body.ok_or("message body missing")?;
        assert!(
            message_body
                .rich_text
                .is_some_and(|rich| rich.elements.is_empty())
        );
        let content = PrivateFileContent::decode(
            message_body
                .content
                .ok_or("file content missing")?
                .as_slice(),
        )?;
        let file = content.file.ok_or("file missing")?;
        assert_eq!(
            (file.file_type, file.subcommand, file.danger_level),
            (Some(0), Some(1), Some(0))
        );
        assert_eq!(
            (file.file_id.as_deref(), file.file_hash.as_deref()),
            (Some("file-id"), Some("addon"))
        );
        assert_eq!((file.file_size, file.expire_at), (Some(3), Some(700_000)));
        Ok(())
    }

    #[test]
    fn final_acknowledgement_requires_qq_correlation() {
        let accepted = SendResponse {
            result: 0,
            timestamp: 0,
            group_sequence: None,
            private_sequence: 1,
        }
        .encode_to_vec();
        assert!(validate_private_file_message_response(&accepted).is_ok());

        let rejected = SendResponse {
            result: 1,
            timestamp: 0,
            group_sequence: None,
            private_sequence: 0,
        }
        .encode_to_vec();
        assert!(validate_private_file_message_response(&rejected).is_err());
        assert!(validate_private_file_message_response(&[]).is_err());
    }

    #[test]
    fn invalid_final_message_input_fails_closed() {
        assert!(
            encode_private_file_message(&PrivateFileMessageInput {
                recipient_uid: "",
                file_id: "file-id",
                file_hash: "addon",
                file_name: "a.bin",
                file_size: 3,
                md5: &[7; 16],
                expire_at: 200,
                client_sequence: 8,
                random: 9,
                unix_seconds: 100,
            })
            .is_err()
        );
    }
}
