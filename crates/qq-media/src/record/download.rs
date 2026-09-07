use prost::Message;

use crate::MediaError;
use crate::download::build_url;
use crate::image_proto::{
    ClientMeta, CommonHead, DownloadExtension, DownloadRequest, GroupTarget, RawMessageBody,
    RawMessageInfo, RequestHead, RichRequest, RichResponse, Scene, VoiceDownloadExtension,
};

const COMMAND: &str = "OidbSvcTrpcTcp.0x126e_200";
const OIDB_COMMAND: u32 = 0x126e;
const SUBCOMMAND: u32 = 200;
const MAX_MESSAGE_INFO_BYTES: usize = 1024 * 1024;

/// One QQ group-record download request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordDownloadRequest {
    body: Vec<u8>,
}

impl RecordDownloadRequest {
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

/// Encodes the group-record URL lookup for QQ-created message material.
///
/// # Errors
///
/// Returns an error for a zero group or malformed/excessive message material.
pub fn encode_group_record_download_request(
    message_info: &[u8],
    group_id: u32,
) -> Result<RecordDownloadRequest, MediaError> {
    if group_id == 0 || message_info.is_empty() || message_info.len() > MAX_MESSAGE_INFO_BYTES {
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
                business_type: 3,
                kind: 2,
                direct: None,
                group: Some(GroupTarget {
                    group_code: group_id,
                }),
            }),
            client: Some(ClientMeta { agent_type: 2 }),
        }),
        upload: None,
        download: Some(DownloadRequest {
            index: Some(index),
            extension: Some(DownloadExtension {
                picture: None,
                voice: Some(VoiceDownloadExtension {}),
            }),
        }),
        download_rkey: None,
    }
    .encode_to_vec();
    let body = qq_wire::encode_oidb_request(OIDB_COMMAND, SUBCOMMAND, &request, 0)
        .map_err(|_error| MediaError::ReferenceRejected)?;
    Ok(RecordDownloadRequest { body })
}

/// Parses one successful group-record download response into an HTTPS URL.
///
/// # Errors
///
/// Returns an error for rejected, malformed, or unsafe URL material.
pub fn parse_group_record_download_response(input: &[u8]) -> Result<String, MediaError> {
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
    build_url(response.download.ok_or(MediaError::RemoteRejected)?)
}

#[cfg(test)]
mod tests {
    use prost::Message;

    use super::{encode_group_record_download_request, parse_group_record_download_response};
    use crate::image_proto::{
        DownloadInfo, DownloadResponse, IndexNode, RawMessageBody, RawMessageInfo, ResponseHead,
        RichRequest, RichResponse,
    };

    #[test]
    fn request_preserves_generated_index_and_group_scene() -> Result<(), Box<dyn std::error::Error>>
    {
        let index = IndexNode {
            uuid: "voice-uuid".to_owned(),
            store_id: 1,
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
        let request = encode_group_record_download_request(&message_info, 123)?;
        let outer = qq_wire::decode_oidb_request(request.body())?;
        assert_eq!((outer.command(), outer.subcommand()), (0x126e, 200));
        let rich = RichRequest::decode(outer.body())?;
        let scene = rich
            .head
            .and_then(|head| head.scene)
            .ok_or("scene missing")?;
        assert_eq!(
            (scene.request_type, scene.business_type, scene.kind),
            (2, 3, 2)
        );
        assert_eq!(scene.group.ok_or("group missing")?.group_code, 123);
        Ok(())
    }

    #[test]
    fn response_uses_shared_safe_url_projection() -> Result<(), Box<dyn std::error::Error>> {
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
                    path: "/download/voice".to_owned(),
                }),
            }),
            download_rkey: None,
        }
        .encode_to_vec();
        let outer = qq_wire::encode_oidb_request(0x126e, 200, &rich, 0)?;
        assert_eq!(
            parse_group_record_download_response(&outer)?,
            "https://multimedia.example.qq.com/download/voice?rkey=opaque"
        );
        Ok(())
    }
}
