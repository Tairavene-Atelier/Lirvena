use std::net::IpAddr;

use prost::Message;

use crate::{ControlError, ControlRequest, request, validate_uid};

const MAX_FILE_TOKEN_BYTES: usize = 4 * 1024;
const MAX_RESPONSE_BYTES: usize = 2 * 1024 * 1024;
const MAX_URL_PATH_BYTES: usize = 8 * 1024;

/// Encodes a private-file download URL query.
///
/// # Errors
///
/// Returns an error for an invalid recipient UID, file identifier, or hash.
pub fn private_file_url(
    recipient_uid: &str,
    file_id: &str,
    file_hash: &str,
) -> Result<ControlRequest, ControlError> {
    validate_uid(recipient_uid)?;
    if !valid_token(file_id) || !valid_token(file_hash) {
        return Err(ControlError);
    }
    request(
        0x0e37,
        1200,
        "OidbSvcTrpcTcp.0xe37_1200",
        None,
        &PrivateFileUrlRequest {
            subcommand: 1200,
            compatibility_one: 1,
            body: Some(PrivateFileUrlBody {
                recipient_uid: recipient_uid.to_owned(),
                file_id: file_id.to_owned(),
                kind: 2,
                file_hash: file_hash.to_owned(),
                compatibility_two: 0,
            }),
            compatibility_three: 3,
            compatibility_four: 103,
            compatibility_five: 1,
            trailer: vec![0xc0, 0x85, 0x2c, 0x01],
        },
    )
}

/// Parses the QQ private-file location response.
///
/// The returned HTTP URL is passed to `OneBot` without being fetched by Lirvena.
///
/// # Errors
///
/// Returns an error for rejected, malformed, incomplete, or unsafe address material.
pub fn parse_private_file_url_response(input: &[u8]) -> Result<String, ControlError> {
    if input.len() > MAX_RESPONSE_BYTES {
        return Err(ControlError);
    }
    let outer = qq_wire::decode_oidb_response(input).map_err(|_error| ControlError)?;
    if outer.error_code() != 0 {
        return Err(ControlError);
    }
    let response = PrivateFileUrlResponse::decode(outer.body()).map_err(|_error| ControlError)?;
    let location = response
        .body
        .and_then(|body| body.result)
        .ok_or(ControlError)?;
    let host = url_host(&location.server)?;
    let port = u16::try_from(location.port)
        .ok()
        .filter(|port| *port != 0)
        .ok_or(ControlError)?;
    if location.path.is_empty()
        || location.path.len() > MAX_URL_PATH_BYTES
        || !location.path.starts_with('/')
        || !location.path.contains('?')
        || location.path.chars().any(char::is_control)
    {
        return Err(ControlError);
    }
    Ok(format!("http://{host}:{port}{}&isthumb=0", location.path))
}

fn valid_token(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_FILE_TOKEN_BYTES
        && value.trim() == value
        && !value.chars().any(char::is_control)
}

fn url_host(value: &str) -> Result<String, ControlError> {
    if value.is_empty() || value.len() > 253 || value.chars().any(char::is_control) {
        return Err(ControlError);
    }
    if let Ok(address) = value.parse::<IpAddr>() {
        return Ok(match address {
            IpAddr::V4(value) => value.to_string(),
            IpAddr::V6(value) => format!("[{value}]"),
        });
    }
    let valid = value.split('.').all(|label| {
        !label.is_empty()
            && label.len() <= 63
            && !label.starts_with('-')
            && !label.ends_with('-')
            && label
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    });
    if valid {
        Ok(value.to_owned())
    } else {
        Err(ControlError)
    }
}

#[derive(Clone, PartialEq, Message)]
struct PrivateFileUrlRequest {
    #[prost(uint32, tag = "1")]
    subcommand: u32,
    #[prost(int32, tag = "2")]
    compatibility_one: i32,
    #[prost(message, optional, tag = "14")]
    body: Option<PrivateFileUrlBody>,
    #[prost(int32, tag = "101")]
    compatibility_three: i32,
    #[prost(int32, tag = "102")]
    compatibility_four: i32,
    #[prost(int32, tag = "200")]
    compatibility_five: i32,
    #[prost(bytes = "vec", tag = "99999")]
    trailer: Vec<u8>,
}

#[derive(Clone, PartialEq, Message)]
struct PrivateFileUrlBody {
    #[prost(string, tag = "10")]
    recipient_uid: String,
    #[prost(string, tag = "20")]
    file_id: String,
    #[prost(int32, tag = "30")]
    kind: i32,
    #[prost(string, tag = "60")]
    file_hash: String,
    #[prost(int32, tag = "601")]
    compatibility_two: i32,
}

#[derive(Clone, PartialEq, Message)]
struct PrivateFileUrlResponse {
    #[prost(message, optional, tag = "14")]
    body: Option<PrivateFileUrlResponseBody>,
}

#[derive(Clone, PartialEq, Message)]
struct PrivateFileUrlResponseBody {
    #[prost(message, optional, tag = "30")]
    result: Option<PrivateFileLocation>,
}

#[derive(Clone, PartialEq, Message)]
struct PrivateFileLocation {
    #[prost(string, tag = "20")]
    server: String,
    #[prost(uint32, tag = "40")]
    port: u32,
    #[prost(string, tag = "50")]
    path: String,
}

#[cfg(test)]
mod tests {
    use prost::Message;

    use super::{
        PrivateFileLocation, PrivateFileUrlRequest, PrivateFileUrlResponse,
        PrivateFileUrlResponseBody, parse_private_file_url_response, private_file_url,
    };

    #[test]
    fn request_preserves_all_frozen_fields() -> Result<(), Box<dyn std::error::Error>> {
        let request = private_file_url("u_peer", "file-id", "file-hash")?;
        assert_eq!(request.command(), "OidbSvcTrpcTcp.0xe37_1200");
        assert_eq!(request.signing_operation(), None);
        let outer = qq_wire::decode_oidb_request(request.body())?;
        assert_eq!((outer.command(), outer.subcommand()), (0x0e37, 1200));
        let body = PrivateFileUrlRequest::decode(outer.body())?;
        assert_eq!(
            (
                body.subcommand,
                body.compatibility_one,
                body.compatibility_three,
                body.compatibility_four,
                body.compatibility_five
            ),
            (1200, 1, 3, 103, 1)
        );
        assert_eq!(body.trailer, [0xc0, 0x85, 0x2c, 0x01]);
        let payload = body.body.ok_or("body missing")?;
        assert_eq!(payload.recipient_uid, "u_peer");
        assert_eq!(payload.file_id, "file-id");
        assert_eq!(payload.file_hash, "file-hash");
        assert_eq!((payload.kind, payload.compatibility_two), (2, 0));
        Ok(())
    }

    #[test]
    fn response_returns_the_qq_http_location() -> Result<(), Box<dyn std::error::Error>> {
        let response = PrivateFileUrlResponse {
            body: Some(PrivateFileUrlResponseBody {
                result: Some(PrivateFileLocation {
                    server: "203.0.113.8".to_owned(),
                    port: 8080,
                    path: "/download?token=opaque".to_owned(),
                }),
            }),
        }
        .encode_to_vec();
        let outer = qq_wire::encode_oidb_request(0x0e37, 1200, &response, 0)?;
        assert_eq!(
            parse_private_file_url_response(&outer)?,
            "http://203.0.113.8:8080/download?token=opaque&isthumb=0"
        );
        Ok(())
    }

    #[test]
    fn unsafe_or_rejected_locations_fail_closed() -> Result<(), Box<dyn std::error::Error>> {
        assert!(private_file_url("", "file", "hash").is_err());
        let response = PrivateFileUrlResponse {
            body: Some(PrivateFileUrlResponseBody {
                result: Some(PrivateFileLocation {
                    server: "bad/host".to_owned(),
                    port: 80,
                    path: "/download?token=opaque".to_owned(),
                }),
            }),
        }
        .encode_to_vec();
        let rejected = qq_wire::encode_oidb_request(0x0e37, 1200, &response, 0)?;
        assert!(parse_private_file_url_response(&rejected).is_err());
        Ok(())
    }
}
