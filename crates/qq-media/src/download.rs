use prost::Message;

use crate::MediaError;
use crate::image::valid_uid;
use crate::image_proto::{
    ClientMeta, CommonHead, DirectTarget, DownloadExtension, DownloadRequest, DownloadResponse,
    PictureDownloadExtension, RawMessageBody, RawMessageInfo, RequestHead, RichRequest,
    RichResponse, Scene,
};

const COMMAND: &str = "OidbSvcTrpcTcp.0x11c5_200";
const OIDB_COMMAND: u32 = 0x11c5;
const SUBCOMMAND: u32 = 200;
const MAX_MESSAGE_INFO_BYTES: usize = 1024 * 1024;
const MAX_URL_PART_BYTES: usize = 8 * 1024;

/// One bounded QQ request for the download URL of an uploaded image.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImageDownloadRequest {
    body: Vec<u8>,
}

impl ImageDownloadRequest {
    /// Returns the audited QQ command.
    #[must_use]
    pub const fn command(&self) -> &'static str {
        COMMAND
    }

    /// Returns the encoded OIDB body.
    #[must_use]
    pub fn body(&self) -> &[u8] {
        &self.body
    }
}

/// Encodes the direct-image download lookup for Tencent-created message material.
///
/// # Errors
///
/// Returns an error for malformed message material or an invalid account UID.
pub fn encode_image_download_request(
    message_info: &[u8],
    account_uid: &str,
) -> Result<ImageDownloadRequest, MediaError> {
    if message_info.is_empty()
        || message_info.len() > MAX_MESSAGE_INFO_BYTES
        || !valid_uid(account_uid)
    {
        return Err(MediaError::ReferenceRejected);
    }
    let info = RawMessageInfo::decode(message_info).map_err(|_error| MediaError::RemoteRejected)?;
    let body = info.bodies.first().ok_or(MediaError::RemoteRejected)?;
    let body =
        RawMessageBody::decode(body.as_slice()).map_err(|_error| MediaError::RemoteRejected)?;
    let index = crate::image_proto::IndexNode::decode(body.index.as_slice())
        .map_err(|_error| MediaError::RemoteRejected)?;
    if index.uuid.is_empty() {
        return Err(MediaError::RemoteRejected);
    }
    let request = RichRequest {
        head: Some(RequestHead {
            common: Some(CommonHead {
                request_id: 1,
                command: SUBCOMMAND,
            }),
            scene: Some(Scene {
                request_type: 2,
                business_type: 1,
                kind: 1,
                direct: Some(DirectTarget {
                    account_type: 2,
                    uid: account_uid.to_owned(),
                }),
                group: None,
            }),
            client: Some(ClientMeta { agent_type: 2 }),
        }),
        download_rkey: None,
        upload: None,
        download: Some(DownloadRequest {
            index: Some(index),
            extension: Some(DownloadExtension {
                picture: Some(PictureDownloadExtension {}),
            }),
        }),
    }
    .encode_to_vec();
    let body = qq_wire::encode_oidb_request(OIDB_COMMAND, SUBCOMMAND, &request, 0)
        .map_err(|_error| MediaError::ReferenceRejected)?;
    Ok(ImageDownloadRequest { body })
}

/// Parses one successful image download response into an HTTPS URL.
///
/// # Errors
///
/// Returns an error for rejected, malformed or unsafe URL material.
pub fn parse_image_download_response(input: &[u8]) -> Result<String, MediaError> {
    let outer =
        qq_wire::decode_oidb_response(input).map_err(|_error| MediaError::RemoteRejected)?;
    if outer.error_code() != 0 {
        return Err(MediaError::RemoteRejected);
    }
    let response =
        RichResponse::decode(outer.body()).map_err(|_error| MediaError::RemoteRejected)?;
    if response
        .head
        .as_ref()
        .is_some_and(|head| head.return_code != 0)
    {
        return Err(MediaError::RemoteRejected);
    }
    let download = response.download.ok_or(MediaError::RemoteRejected)?;
    build_url(download)
}

fn build_url(download: DownloadResponse) -> Result<String, MediaError> {
    let info = download.info.ok_or(MediaError::RemoteRejected)?;
    if !valid_domain(&info.domain)
        || !valid_url_part(&info.path)
        || !valid_url_part(&download.rkey_parameter)
        || !info.path.starts_with('/')
    {
        return Err(MediaError::RemoteRejected);
    }
    Ok(format!(
        "https://{}{}{}",
        info.domain, info.path, download.rkey_parameter
    ))
}

fn valid_domain(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 253
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'.' || byte == b'-')
}

fn valid_url_part(value: &str) -> bool {
    value.len() <= MAX_URL_PART_BYTES
        && value.trim() == value
        && !value.chars().any(char::is_control)
}

#[cfg(test)]
mod tests {
    use prost::Message;

    use super::{encode_image_download_request, parse_image_download_response};
    use crate::image_proto::{
        DownloadInfo, DownloadResponse, IndexNode, RawMessageBody, RawMessageInfo, ResponseHead,
        RichRequest, RichResponse,
    };

    #[test]
    fn request_preserves_the_complete_uploaded_index() -> Result<(), Box<dyn std::error::Error>> {
        let index = IndexNode {
            uuid: "image-uuid".to_owned(),
            store_id: 1,
            upload_time: 2,
            ttl: 3,
            sub_type: 4,
            ..IndexNode::default()
        };
        let message_info = RawMessageInfo {
            bodies: vec![
                RawMessageBody {
                    index: index.encode_to_vec(),
                }
                .encode_to_vec(),
            ],
            business: Vec::new(),
        }
        .encode_to_vec();
        let request = encode_image_download_request(&message_info, "u_self")?;
        assert_eq!(request.command(), "OidbSvcTrpcTcp.0x11c5_200");
        let outer = qq_wire::decode_oidb_request(request.body())?;
        assert_eq!((outer.command(), outer.subcommand()), (0x11c5, 200));
        let rich = RichRequest::decode(outer.body())?;
        assert_eq!(rich.download.and_then(|value| value.index), Some(index));
        Ok(())
    }

    #[test]
    fn response_requires_safe_https_components() -> Result<(), Box<dyn std::error::Error>> {
        let rich = RichResponse {
            head: Some(ResponseHead {
                return_code: 0,
                message: String::new(),
            }),
            upload: None,
            download: Some(DownloadResponse {
                rkey_parameter: "?rkey=opaque".to_owned(),
                info: Some(DownloadInfo {
                    domain: "multimedia.example.qq.com".to_owned(),
                    path: "/download/image".to_owned(),
                }),
            }),
            download_rkey: None,
        }
        .encode_to_vec();
        let outer = qq_wire::encode_oidb_request(0x11c5, 200, &rich, 0)?;
        assert_eq!(
            parse_image_download_response(&outer)?,
            "https://multimedia.example.qq.com/download/image?rkey=opaque"
        );
        Ok(())
    }
}
