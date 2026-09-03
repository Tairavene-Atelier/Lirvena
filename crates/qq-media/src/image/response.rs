use crate::{MediaError, MediaTarget, RichMediaUploadPlan};

/// Parses one bounded image metadata response without re-encoding Tencent's
/// modern or compatibility message material.
///
/// # Errors
///
/// Returns an error for rejected, malformed, incomplete, or unbounded material.
pub fn parse_image_metadata_response(
    input: &[u8],
    target: &MediaTarget<'_>,
) -> Result<RichMediaUploadPlan, MediaError> {
    RichMediaUploadPlan::parse(
        input,
        match target {
            MediaTarget::Direct(_) => 1_003,
            MediaTarget::Group(_) => 1_004,
        },
    )
}

#[cfg(test)]
mod tests {
    use prost::Message;

    use super::parse_image_metadata_response;
    use crate::MediaTarget;
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
        let plan = parse_image_metadata_response(&outer, &MediaTarget::Group(42))?;
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
        let plan = parse_image_metadata_response(&outer, &MediaTarget::Direct("u_target"))?;
        assert_eq!(plan.message_info(), [0x08, 0x01]);
        assert!(plan.compatibility_message().is_empty());
        assert!(plan.highway_extension().is_none());
        Ok(())
    }
}
