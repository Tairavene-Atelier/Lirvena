use prost::Message;

use crate::MediaError;
use crate::image_proto::{
    ClientMeta, CommonHead, DownloadRkeyRequest, RequestHead, RichRequest, RichResponse, Scene,
};

const COMMAND: &str = "OidbSvcTrpcTcp.0x9067_202";
const OIDB_COMMAND: u32 = 0x9067;
const SUBCOMMAND: u32 = 202;
const MAX_RKEYS: usize = 16;
const MAX_RKEY_BYTES: usize = 8 * 1024;

/// QQ media scope associated with an rkey.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MediaRkeyKind {
    /// Direct-message media.
    Private,
    /// Group media.
    Group,
}

/// One bounded media rkey returned by QQ.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MediaRkey {
    /// Media scope reported by QQ.
    pub kind: MediaRkeyKind,
    /// Opaque rkey query material.
    pub value: String,
    /// Optional creation time reported by QQ.
    pub created_at: Option<u32>,
    /// Lifetime in seconds reported by QQ.
    pub ttl_seconds: u64,
}

/// One encoded QQ media-rkey request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MediaRkeyRequest {
    body: Vec<u8>,
}

impl MediaRkeyRequest {
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

/// Encodes the frozen Linux media-rkey query.
///
/// # Errors
///
/// Returns an error if the bounded OIDB envelope cannot be encoded.
pub fn encode_media_rkey_request() -> Result<MediaRkeyRequest, MediaError> {
    let inner = RichRequest {
        head: Some(RequestHead {
            common: Some(CommonHead {
                request_id: 1,
                command: SUBCOMMAND,
            }),
            scene: Some(Scene {
                request_type: 2,
                business_type: 1,
                kind: 0,
                direct: None,
                group: None,
            }),
            client: Some(ClientMeta { agent_type: 2 }),
        }),
        upload: None,
        download: None,
        download_rkey: Some(DownloadRkeyRequest {
            types: vec![10, 20, 2],
        }),
    }
    .encode_to_vec();
    let body = qq_wire::encode_oidb_request(OIDB_COMMAND, SUBCOMMAND, &inner, 1)
        .map_err(|_error| MediaError::ReferenceRejected)?;
    Ok(MediaRkeyRequest { body })
}

/// Parses one bounded media-rkey response.
///
/// # Errors
///
/// Returns an error for rejected, malformed, missing, unknown, or excessive material.
pub fn parse_media_rkey_response(input: &[u8]) -> Result<Vec<MediaRkey>, MediaError> {
    let outer =
        qq_wire::decode_oidb_response(input).map_err(|_error| MediaError::RemoteRejected)?;
    if outer.error_code() != 0 {
        return Err(MediaError::RemoteRejected);
    }
    let response =
        RichResponse::decode(outer.body()).map_err(|_error| MediaError::RemoteRejected)?;
    let head = response.head.ok_or(MediaError::RemoteRejected)?;
    if head.return_code != 0 {
        return Err(MediaError::RemoteRejected);
    }
    let values = response
        .download_rkey
        .ok_or(MediaError::RemoteRejected)?
        .values;
    if values.is_empty() || values.len() > MAX_RKEYS {
        return Err(MediaError::RemoteRejected);
    }
    values
        .into_iter()
        .map(|value| {
            if value.value.is_empty()
                || value.value.len() > MAX_RKEY_BYTES
                || value.value.trim() != value.value
                || value.value.chars().any(char::is_control)
            {
                return Err(MediaError::RemoteRejected);
            }
            let kind = match value.kind {
                Some(10) => MediaRkeyKind::Private,
                Some(2 | 20) => MediaRkeyKind::Group,
                Some(_) | None => return Err(MediaError::RemoteRejected),
            };
            Ok(MediaRkey {
                kind,
                value: value.value,
                created_at: value.created_at,
                ttl_seconds: value.ttl_seconds,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use prost::Message;

    use super::{MediaRkeyKind, encode_media_rkey_request, parse_media_rkey_response};
    use crate::image_proto::{
        DownloadRkeyResponse, ResponseHead, RichRequest, RichResponse, RkeyInfo,
    };

    #[test]
    fn request_preserves_frozen_head_types_and_uid_reservation()
    -> Result<(), Box<dyn std::error::Error>> {
        let request = encode_media_rkey_request()?;
        assert_eq!(request.command(), "OidbSvcTrpcTcp.0x9067_202");
        let outer = qq_wire::decode_oidb_request(request.body())?;
        assert_eq!((outer.command(), outer.subcommand()), (0x9067, 202));
        assert_eq!(outer.reserved(), 1);
        let request = RichRequest::decode(outer.body())?;
        let head = request.head.ok_or("head missing")?;
        let common = head.common.ok_or("common head missing")?;
        let scene = head.scene.ok_or("scene missing")?;
        assert_eq!((common.request_id, common.command), (1, 202));
        assert_eq!(
            (scene.request_type, scene.business_type, scene.kind),
            (2, 1, 0)
        );
        assert_eq!(head.client.ok_or("client missing")?.agent_type, 2);
        assert_eq!(
            request.download_rkey.ok_or("rkey request missing")?.types,
            [10, 20, 2]
        );
        Ok(())
    }

    #[test]
    fn response_projects_only_known_bounded_scopes() -> Result<(), Box<dyn std::error::Error>> {
        let inner = RichResponse {
            head: Some(ResponseHead {
                return_code: 0,
                message: String::new(),
            }),
            upload: None,
            download: None,
            download_rkey: Some(DownloadRkeyResponse {
                values: vec![
                    RkeyInfo {
                        value: "?rkey=private".to_owned(),
                        ttl_seconds: 600,
                        store_id: 0,
                        created_at: Some(100),
                        kind: Some(10),
                    },
                    RkeyInfo {
                        value: "?rkey=group".to_owned(),
                        ttl_seconds: 900,
                        store_id: 0,
                        created_at: None,
                        kind: Some(20),
                    },
                ],
            }),
        }
        .encode_to_vec();
        let outer = qq_wire::encode_oidb_request(0x9067, 202, &inner, 0)?;
        let values = parse_media_rkey_response(&outer)?;
        assert_eq!(values[0].kind, MediaRkeyKind::Private);
        assert_eq!(values[0].created_at, Some(100));
        assert_eq!(values[1].kind, MediaRkeyKind::Group);
        Ok(())
    }

    #[test]
    fn unknown_or_rejected_responses_fail_closed() -> Result<(), Box<dyn std::error::Error>> {
        let inner = RichResponse {
            head: Some(ResponseHead {
                return_code: 0,
                message: String::new(),
            }),
            upload: None,
            download: None,
            download_rkey: Some(DownloadRkeyResponse {
                values: vec![RkeyInfo {
                    value: "?rkey=opaque".to_owned(),
                    ttl_seconds: 1,
                    store_id: 0,
                    created_at: None,
                    kind: None,
                }],
            }),
        }
        .encode_to_vec();
        let outer = qq_wire::encode_oidb_request(0x9067, 202, &inner, 0)?;
        assert!(parse_media_rkey_response(&outer).is_err());
        Ok(())
    }
}
