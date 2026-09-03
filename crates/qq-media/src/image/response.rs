use prost::Message;

use super::{ImageTarget, ImageUploadPlan, decode_hex};
use crate::MediaError;
use crate::image_proto::{
    HighwayAddress, HighwayDomain, HighwayExtension, HighwayHashes, HighwayNetwork, IndexNode,
    RawMessageBody, RawMessageInfo, RichResponse, UploadResponse,
};

const MAX_RESPONSE_BYTES: usize = 2 * 1024 * 1024;
const MAX_MESSAGE_INFO_BYTES: usize = 1024 * 1024;
const MAX_COMPATIBILITY_BYTES: usize = 512 * 1024;
const MAX_UPLOAD_KEY_BYTES: usize = 4 * 1024;
const MAX_MESSAGE_BODIES: usize = 16;
const MAX_NETWORK_ADDRESSES: usize = 32;
const HIGHWAY_BLOCK_BYTES: u32 = 1024 * 1024;

/// Parses one bounded image metadata response without re-encoding Tencent's
/// modern or compatibility message material.
///
/// # Errors
///
/// Returns an error for rejected, malformed, incomplete, or unbounded material.
pub fn parse_image_metadata_response(
    input: &[u8],
    target: &ImageTarget<'_>,
) -> Result<ImageUploadPlan, MediaError> {
    if input.len() > MAX_RESPONSE_BYTES {
        return Err(MediaError::RemoteRejected);
    }
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
    let upload = response.upload.ok_or(MediaError::RemoteRejected)?;
    validate_response(&upload)?;
    let highway_extension = if upload.upload_key.is_empty() {
        None
    } else {
        Some(build_highway_extension(&upload)?)
    };
    Ok(ImageUploadPlan {
        message_info: upload.message_info,
        compatibility_message: upload.compatibility_message,
        highway_extension,
        command_id: match target {
            ImageTarget::Direct(_) => 1_003,
            ImageTarget::Group(_) => 1_004,
        },
    })
}

fn validate_response(upload: &UploadResponse) -> Result<(), MediaError> {
    if upload.message_info.is_empty()
        || upload.message_info.len() > MAX_MESSAGE_INFO_BYTES
        || upload.compatibility_message.len() > MAX_COMPATIBILITY_BYTES
        || upload.upload_key.len() > MAX_UPLOAD_KEY_BYTES
        || upload.ipv4.len() > MAX_NETWORK_ADDRESSES
        || upload.upload_key.chars().any(char::is_control)
    {
        return Err(MediaError::RemoteRejected);
    }
    Ok(())
}

fn build_highway_extension(upload: &UploadResponse) -> Result<Vec<u8>, MediaError> {
    let message = RawMessageInfo::decode(upload.message_info.as_slice())
        .map_err(|_error| MediaError::RemoteRejected)?;
    if message.bodies.is_empty() || message.bodies.len() > MAX_MESSAGE_BODIES {
        return Err(MediaError::RemoteRejected);
    }
    let body = RawMessageBody::decode(message.bodies[0].as_slice())
        .map_err(|_error| MediaError::RemoteRejected)?;
    let index =
        IndexNode::decode(body.index.as_slice()).map_err(|_error| MediaError::RemoteRejected)?;
    let info = index.info.ok_or(MediaError::RemoteRejected)?;
    let sha1 = decode_hex::<20>(&info.sha1)?;
    if index.uuid.is_empty() || index.uuid.len() > 512 || index.uuid.chars().any(char::is_control) {
        return Err(MediaError::RemoteRejected);
    }
    let addresses = upload
        .ipv4
        .iter()
        .filter_map(|value| {
            let port = u16::try_from(value.external_port).ok()?;
            if port == 0 {
                return None;
            }
            Some(HighwayAddress {
                domain: Some(HighwayDomain {
                    enabled: true,
                    address: std::net::Ipv4Addr::from(value.external_address.to_le_bytes())
                        .to_string(),
                }),
                port: u32::from(port),
            })
        })
        .collect();
    Ok(HighwayExtension {
        uuid: index.uuid,
        upload_key: upload.upload_key.clone(),
        network: Some(HighwayNetwork { addresses }),
        message_bodies: message.bodies,
        block_size: HIGHWAY_BLOCK_BYTES,
        hashes: Some(HighwayHashes {
            sha1: vec![sha1.to_vec()],
        }),
    }
    .encode_to_vec())
}

#[cfg(test)]
mod tests {
    use prost::Message;

    use super::parse_image_metadata_response;
    use crate::ImageTarget;
    use crate::image_proto::{
        FileInfo, IndexNode, Ipv4, RawMessageBody, RawMessageInfo, ResponseHead, RichResponse,
        UploadResponse,
    };

    #[test]
    fn response_preserves_message_material_and_builds_upload_extension()
    -> Result<(), Box<dyn std::error::Error>> {
        let index = IndexNode {
            info: Some(FileInfo {
                sha1: "00112233445566778899aabbccddeeff00112233".to_owned(),
                ..FileInfo::default()
            }),
            uuid: "uuid".to_owned(),
        }
        .encode_to_vec();
        let message_info = RawMessageInfo {
            bodies: vec![RawMessageBody { index }.encode_to_vec()],
            business: vec![0xaa],
        }
        .encode_to_vec();
        let rich = RichResponse {
            head: Some(ResponseHead {
                return_code: 0,
                message: String::new(),
            }),
            upload: Some(UploadResponse {
                upload_key: "ukey".to_owned(),
                ipv4: vec![Ipv4 {
                    external_address: u32::from_le_bytes([1, 2, 3, 4]),
                    external_port: 80,
                }],
                message_info: message_info.clone(),
                compatibility_message: vec![0x08, 0x01],
            }),
        }
        .encode_to_vec();
        let outer = qq_wire::encode_oidb_request(0x11c4, 100, &rich, 0)?;
        let plan = parse_image_metadata_response(&outer, &ImageTarget::Group(42))?;
        assert_eq!(plan.message_info(), message_info);
        assert_eq!(plan.compatibility_message(), [0x08, 0x01]);
        assert!(plan.highway_extension().is_some());
        assert_eq!(plan.command_id(), 1_004);
        Ok(())
    }

    #[test]
    fn fast_upload_may_omit_legacy_compatibility_material() -> Result<(), Box<dyn std::error::Error>>
    {
        let rich = RichResponse {
            head: Some(ResponseHead {
                return_code: 0,
                message: String::new(),
            }),
            upload: Some(UploadResponse {
                upload_key: String::new(),
                ipv4: Vec::new(),
                message_info: vec![0x08, 0x01],
                compatibility_message: Vec::new(),
            }),
        }
        .encode_to_vec();
        let outer = qq_wire::encode_oidb_request(0x11c5, 100, &rich, 0)?;
        let plan = parse_image_metadata_response(&outer, &ImageTarget::Direct("u_target"))?;
        assert_eq!(plan.message_info(), [0x08, 0x01]);
        assert!(plan.compatibility_message().is_empty());
        assert!(plan.highway_extension().is_none());
        Ok(())
    }
}
