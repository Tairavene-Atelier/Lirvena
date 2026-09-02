use prost::Message;
use qq_wire::{decode_oidb_response, encode_oidb_request};

const MAX_UID_BYTES: usize = 128;

/// Opaque UID-directory codec error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UserDirectoryError;

impl core::fmt::Display for UserDirectoryError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("QQ user directory data is invalid")
    }
}

impl std::error::Error for UserDirectoryError {}

/// Encodes one bounded Linux NT UID-to-UIN lookup.
///
/// # Errors
///
/// Returns an error for an empty, excessive, or unsafe UID.
pub fn encode_user_lookup_request(uid: &str) -> Result<Vec<u8>, UserDirectoryError> {
    validate_uid(uid)?;
    let body = UserLookupRequest {
        uid: uid.to_owned(),
        field2: 0,
        properties: vec![PropertyRequest { key: 20_002 }],
    }
    .encode_to_vec();
    encode_oidb_request(0x0fe1, 2, &body, 0).map_err(|_error| UserDirectoryError)
}

/// Parses one successful bounded UID-to-UIN lookup response.
///
/// # Errors
///
/// Returns an error for malformed data, a rejected response, or a zero UIN.
pub fn parse_user_lookup(input: &[u8]) -> Result<u32, UserDirectoryError> {
    let outer = decode_oidb_response(input).map_err(|_error| UserDirectoryError)?;
    if outer.error_code() != 0 {
        return Err(UserDirectoryError);
    }
    let response = UserLookupResponse::decode(outer.body()).map_err(|_error| UserDirectoryError)?;
    let user = response.user.ok_or(UserDirectoryError)?;
    if user.uin == 0 {
        return Err(UserDirectoryError);
    }
    Ok(user.uin)
}

fn validate_uid(uid: &str) -> Result<(), UserDirectoryError> {
    if uid.is_empty() || uid.len() > MAX_UID_BYTES || uid.chars().any(char::is_control) {
        Err(UserDirectoryError)
    } else {
        Ok(())
    }
}

#[derive(Clone, PartialEq, Message)]
struct UserLookupRequest {
    #[prost(string, tag = "1")]
    uid: String,
    #[prost(uint32, tag = "2")]
    field2: u32,
    #[prost(message, repeated, tag = "3")]
    properties: Vec<PropertyRequest>,
}

#[derive(Clone, Copy, PartialEq, Message)]
struct PropertyRequest {
    #[prost(uint32, tag = "1")]
    key: u32,
}

#[derive(Clone, PartialEq, Message)]
struct UserLookupResponse {
    #[prost(message, optional, tag = "1")]
    user: Option<UserLookupBody>,
}

#[derive(Clone, PartialEq, Message)]
struct UserLookupBody {
    #[prost(uint32, tag = "3")]
    uin: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_lookup_is_bounded_and_returns_numeric_identity()
    -> Result<(), Box<dyn std::error::Error>> {
        let request = encode_user_lookup_request("u_target")?;
        let outer = qq_wire::decode_oidb_request(&request)?;
        assert_eq!((outer.command(), outer.subcommand()), (0x0fe1, 2));

        let inner = TestUserLookupResponse {
            user: Some(TestUserLookupBody {
                properties: Some(TestUserProperties {
                    numbers: vec![TestNumberProperty {
                        first: 20_002,
                        second: 7,
                    }],
                }),
                uin: 12_345,
            }),
        }
        .encode_to_vec();
        let response = qq_wire::encode_oidb_request(0x0fe1, 2, &inner, 0)?;
        assert_eq!(parse_user_lookup(&response)?, 12_345);
        assert!(encode_user_lookup_request("").is_err());
        Ok(())
    }

    #[derive(Clone, PartialEq, Message)]
    struct TestUserLookupResponse {
        #[prost(message, optional, tag = "1")]
        user: Option<TestUserLookupBody>,
    }

    #[derive(Clone, PartialEq, Message)]
    struct TestUserLookupBody {
        #[prost(message, optional, tag = "2")]
        properties: Option<TestUserProperties>,
        #[prost(uint32, tag = "3")]
        uin: u32,
    }

    #[derive(Clone, PartialEq, Message)]
    struct TestUserProperties {
        #[prost(message, repeated, tag = "1")]
        numbers: Vec<TestNumberProperty>,
    }

    #[derive(Clone, Copy, PartialEq, Message)]
    struct TestNumberProperty {
        #[prost(uint32, tag = "1")]
        first: u32,
        #[prost(uint32, tag = "2")]
        second: u32,
    }
}
