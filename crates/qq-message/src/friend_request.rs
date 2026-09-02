use prost::Message;

use crate::{MessageClass, MessageDecodeError, MessageEnvelope};

const MAX_UID_BYTES: usize = 128;

/// Authenticated signal that QQ's friend-request directory changed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FriendRequestSignal {
    source_uid: String,
}

impl FriendRequestSignal {
    /// Returns the current Linux NT UID of the applicant.
    #[must_use]
    pub fn source_uid(&self) -> &str {
        &self.source_uid
    }
}

/// Decodes the evidence-backed friend-request Push subtype.
///
/// # Errors
///
/// Returns an error when the known subtype carries malformed or unsafe data.
pub fn decode_friend_request_signal(
    envelope: &MessageEnvelope,
) -> Result<Option<FriendRequestSignal>, MessageDecodeError> {
    if envelope.class() != MessageClass::FriendEvent || envelope.sub_type() != 35 {
        return Ok(None);
    }
    let content = envelope.payload().content().ok_or(MessageDecodeError)?;
    let info = FriendRequestWire::decode(content)
        .map_err(|_error| MessageDecodeError)?
        .info
        .ok_or(MessageDecodeError)?;
    validate_uid(&info.source_uid)?;
    Ok(Some(FriendRequestSignal {
        source_uid: info.source_uid,
    }))
}

fn validate_uid(value: &str) -> Result<(), MessageDecodeError> {
    if value.is_empty() || value.len() > MAX_UID_BYTES || value.chars().any(char::is_control) {
        Err(MessageDecodeError)
    } else {
        Ok(())
    }
}

#[derive(Clone, PartialEq, Message)]
struct FriendRequestWire {
    #[prost(message, optional, tag = "1")]
    info: Option<FriendRequestInfoWire>,
}

#[derive(Clone, PartialEq, Message)]
struct FriendRequestInfoWire {
    #[prost(string, tag = "1")]
    target_uid: String,
    #[prost(string, tag = "2")]
    source_uid: String,
    #[prost(string, tag = "10")]
    comment: String,
    #[prost(string, tag = "11")]
    source: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wire_shape_preserves_request_identity_and_text() -> Result<(), Box<dyn std::error::Error>> {
        let encoded = FriendRequestWire {
            info: Some(FriendRequestInfoWire {
                target_uid: "u_self".to_owned(),
                source_uid: "u_friend".to_owned(),
                comment: "hello".to_owned(),
                source: "search".to_owned(),
            }),
        }
        .encode_to_vec();
        let decoded = FriendRequestWire::decode(encoded.as_slice())?
            .info
            .ok_or("missing friend request")?;
        assert_eq!(decoded.source_uid, "u_friend");
        assert_eq!(decoded.comment, "hello");
        Ok(())
    }
}
