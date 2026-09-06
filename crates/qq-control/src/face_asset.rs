use prost::Message;

use crate::{ControlError, ControlRequest};

const MAX_RESPONSE_BYTES: usize = 2 * 1024 * 1024;
const MAX_FACE_ITEMS: usize = 512;
const MAX_TEXT_BYTES: usize = 1024;

/// Encodes the current-account custom-face listing request.
///
/// The application version must come from the authenticated Ceylith Profile. The kernel release is
/// part of the user-managed synthetic device description.
///
/// # Errors
///
/// Returns an error for invalid account or bounded text fields.
pub fn fetch_custom_faces(
    account_uin: u32,
    kernel_release: &str,
    client_version: &str,
) -> Result<ControlRequest, ControlError> {
    if account_uin == 0 || !valid_text(kernel_release) || !valid_text(client_version) {
        return Err(ControlError);
    }
    Ok(ControlRequest {
        command: "Faceroam.OpReq",
        body: CustomFaceRequest {
            platform: Some(PlatformInfo {
                kind: 1,
                kernel_release: kernel_release.to_owned(),
                client_version: client_version.to_owned(),
            }),
            account_uin,
            operation: 1,
            compatibility: 1,
        }
        .encode_to_vec(),
        signing_operation: None,
    })
}

/// Parses the custom-face response into bounded HTTPS URLs.
///
/// # Errors
///
/// Returns an error for a rejected response or unsafe path components.
pub fn parse_custom_faces_response(
    input: &[u8],
    account_uin: u32,
) -> Result<Vec<String>, ControlError> {
    if input.len() > MAX_RESPONSE_BYTES || account_uin == 0 {
        return Err(ControlError);
    }
    let response = CustomFaceResponse::decode(input).map_err(|_error| ControlError)?;
    if response.result != 0 {
        return Err(ControlError);
    }
    let user = response.user.ok_or(ControlError)?;
    if user.file_names.len() > MAX_FACE_ITEMS {
        return Err(ControlError);
    }
    if user.file_names.is_empty() {
        return Ok(Vec::new());
    }
    if !valid_segment(&user.bucket) {
        return Err(ControlError);
    }
    user.file_names
        .into_iter()
        .map(|file_name| {
            if !valid_segment(&file_name) {
                return Err(ControlError);
            }
            Ok(format!(
                "https://p.qpic.cn/{}/{account_uin}/{file_name}/0",
                user.bucket
            ))
        })
        .collect()
}

/// Encodes one bounded market-face key query.
///
/// # Errors
///
/// Returns an error for an empty, excessive, or unsafe identifier collection.
pub fn fetch_market_face_keys(face_ids: &[String]) -> Result<ControlRequest, ControlError> {
    if face_ids.is_empty()
        || face_ids.len() > MAX_FACE_ITEMS
        || face_ids.iter().any(|value| !valid_text(value))
    {
        return Err(ControlError);
    }
    Ok(ControlRequest {
        command: "BQMallSvc.TabOpReq",
        body: MarketFaceRequest {
            operation: 3,
            query: Some(MarketFaceQuery {
                face_ids: face_ids.to_vec(),
            }),
        }
        .encode_to_vec(),
        signing_operation: None,
    })
}

/// Parses a bounded market-face key response.
///
/// # Errors
///
/// Returns an error for malformed, missing, excessive, or control-bearing key material.
pub fn parse_market_face_keys_response(input: &[u8]) -> Result<Vec<String>, ControlError> {
    if input.len() > MAX_RESPONSE_BYTES {
        return Err(ControlError);
    }
    let response = MarketFaceResponse::decode(input).map_err(|_error| ControlError)?;
    let keys = response.result.ok_or(ControlError)?.keys;
    if keys.is_empty() || keys.len() > MAX_FACE_ITEMS || keys.iter().any(|value| !valid_text(value))
    {
        return Err(ControlError);
    }
    Ok(keys)
}

fn valid_text(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_TEXT_BYTES
        && value.trim() == value
        && !value.chars().any(char::is_control)
}

fn valid_segment(value: &str) -> bool {
    valid_text(value)
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

#[derive(Clone, PartialEq, Message)]
struct CustomFaceRequest {
    #[prost(message, optional, tag = "1")]
    platform: Option<PlatformInfo>,
    #[prost(uint32, tag = "2")]
    account_uin: u32,
    #[prost(uint32, tag = "3")]
    operation: u32,
    #[prost(uint32, tag = "6")]
    compatibility: u32,
}

#[derive(Clone, PartialEq, Message)]
struct PlatformInfo {
    #[prost(uint32, tag = "1")]
    kind: u32,
    #[prost(string, tag = "2")]
    kernel_release: String,
    #[prost(string, tag = "3")]
    client_version: String,
}

#[derive(Clone, PartialEq, Message)]
struct CustomFaceResponse {
    #[prost(uint32, tag = "1")]
    result: u32,
    #[prost(message, optional, tag = "4")]
    user: Option<CustomFaceUser>,
}

#[derive(Clone, PartialEq, Message)]
struct CustomFaceUser {
    #[prost(string, repeated, tag = "1")]
    file_names: Vec<String>,
    #[prost(string, tag = "3")]
    bucket: String,
}

#[derive(Clone, PartialEq, Message)]
struct MarketFaceRequest {
    #[prost(uint32, tag = "1")]
    operation: u32,
    #[prost(message, optional, tag = "5")]
    query: Option<MarketFaceQuery>,
}

#[derive(Clone, PartialEq, Message)]
struct MarketFaceQuery {
    #[prost(string, repeated, tag = "3")]
    face_ids: Vec<String>,
}

#[derive(Clone, PartialEq, Message)]
struct MarketFaceResponse {
    #[prost(message, optional, tag = "5")]
    result: Option<MarketFaceResult>,
}

#[derive(Clone, PartialEq, Message)]
struct MarketFaceResult {
    #[prost(string, repeated, tag = "1")]
    keys: Vec<String>,
}

#[cfg(test)]
mod tests {
    use prost::Message;

    use super::{
        CustomFaceRequest, CustomFaceResponse, CustomFaceUser, MarketFaceRequest,
        MarketFaceResponse, MarketFaceResult, fetch_custom_faces, fetch_market_face_keys,
        parse_custom_faces_response, parse_market_face_keys_response,
    };

    #[test]
    fn custom_face_request_and_urls_preserve_account_and_profile_version()
    -> Result<(), Box<dyn std::error::Error>> {
        let request = fetch_custom_faces(42, "6.8.0", "3.2.32-52194")?;
        assert_eq!(request.command(), "Faceroam.OpReq");
        assert_eq!(request.signing_operation(), None);
        let body = CustomFaceRequest::decode(request.body())?;
        let platform = body.platform.ok_or("platform missing")?;
        assert_eq!(
            (platform.kind, body.operation, body.compatibility),
            (1, 1, 1)
        );
        assert_eq!(platform.kernel_release, "6.8.0");
        assert_eq!(platform.client_version, "3.2.32-52194");
        let response = CustomFaceResponse {
            result: 0,
            user: Some(CustomFaceUser {
                file_names: vec!["AABBCC.png".to_owned()],
                bucket: "face-bucket".to_owned(),
            }),
        }
        .encode_to_vec();
        assert_eq!(
            parse_custom_faces_response(&response, 42)?,
            ["https://p.qpic.cn/face-bucket/42/AABBCC.png/0"]
        );
        Ok(())
    }

    #[test]
    fn market_face_query_and_response_are_bounded() -> Result<(), Box<dyn std::error::Error>> {
        let request = fetch_market_face_keys(&["emoji-one".to_owned(), "emoji-two".to_owned()])?;
        assert_eq!(request.command(), "BQMallSvc.TabOpReq");
        let body = MarketFaceRequest::decode(request.body())?;
        assert_eq!(body.operation, 3);
        assert_eq!(
            body.query.ok_or("query missing")?.face_ids,
            ["emoji-one", "emoji-two"]
        );
        let response = MarketFaceResponse {
            result: Some(MarketFaceResult {
                keys: vec!["key-one".to_owned(), "key-two".to_owned()],
            }),
        }
        .encode_to_vec();
        assert_eq!(
            parse_market_face_keys_response(&response)?,
            ["key-one", "key-two"]
        );
        Ok(())
    }

    #[test]
    fn rejected_or_unsafe_face_material_never_reports_success() {
        assert!(fetch_market_face_keys(&[]).is_err());
        assert!(fetch_custom_faces(0, "6.8.0", "3.2.32-52194").is_err());
        let response = CustomFaceResponse {
            result: 0,
            user: Some(CustomFaceUser {
                file_names: vec!["../escape".to_owned()],
                bucket: "bucket".to_owned(),
            }),
        }
        .encode_to_vec();
        assert!(parse_custom_faces_response(&response, 42).is_err());

        let empty = CustomFaceResponse {
            result: 0,
            user: Some(CustomFaceUser {
                file_names: Vec::new(),
                bucket: String::new(),
            }),
        }
        .encode_to_vec();
        assert_eq!(parse_custom_faces_response(&empty, 42), Ok(Vec::new()));
    }
}
