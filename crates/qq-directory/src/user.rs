use crate::user_wire::{UserSelector, decode_user_response, encode_user_request};

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
    encode_user_request(UserSelector::Uid(uid), &[20_002])
}

/// Parses one successful bounded UID-to-UIN lookup response.
///
/// # Errors
///
/// Returns an error for malformed data, a rejected response, or a zero UIN.
pub fn parse_user_lookup(input: &[u8]) -> Result<u32, UserDirectoryError> {
    Ok(decode_user_response(input)?.uin)
}

#[cfg(test)]
mod tests {
    use super::*;
    use prost::Message;

    #[test]
    fn user_lookup_is_bounded_and_returns_numeric_identity()
    -> Result<(), Box<dyn std::error::Error>> {
        let request = encode_user_lookup_request("u_target")?;
        let outer = qq_wire::decode_oidb_request(&request)?;
        assert_eq!((outer.command(), outer.subcommand()), (0x0fe1, 2));
        assert_eq!(outer.reserved(), 0);

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
