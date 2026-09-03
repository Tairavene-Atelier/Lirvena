use crate::{MediaError, MediaTarget, RichMediaUploadPlan};

/// Parses one bounded video metadata response with its thumbnail continuation.
///
/// # Errors
///
/// Returns an error for rejected, malformed, incomplete, or unbounded material.
pub fn parse_video_metadata_response(
    input: &[u8],
    target: &MediaTarget<'_>,
) -> Result<RichMediaUploadPlan, MediaError> {
    let (main_command, thumbnail_command) = match target {
        MediaTarget::Direct(_) => (1_001, 1_002),
        MediaTarget::Group(_) => (1_005, 1_006),
    };
    RichMediaUploadPlan::parse_with_sub_files(
        input,
        (main_command, true),
        &[(100, thumbnail_command, false)],
    )
}

#[cfg(test)]
mod tests {
    use prost::Message;

    use super::parse_video_metadata_response;
    use crate::MediaTarget;
    use crate::image_proto::{
        FileInfo, IndexNode, Ipv4, RawMessageBody, RawMessageInfo, ResponseHead, RichResponse,
        SubFileInfo, UploadResponse,
    };

    #[test]
    fn group_response_binds_two_highway_continuations() -> Result<(), Box<dyn std::error::Error>> {
        let body = |name: &str, sha1: &str| {
            RawMessageBody {
                index: IndexNode {
                    info: Some(FileInfo {
                        sha1: sha1.to_owned(),
                        ..FileInfo::default()
                    }),
                    uuid: name.to_owned(),
                }
                .encode_to_vec(),
            }
            .encode_to_vec()
        };
        let message_info = RawMessageInfo {
            bodies: vec![
                body("video", "1111111111111111111111111111111111111111"),
                body("thumb", "2222222222222222222222222222222222222222"),
            ],
            business: Vec::new(),
        }
        .encode_to_vec();
        let rich = RichResponse {
            head: Some(ResponseHead {
                return_code: 0,
                message: String::new(),
            }),
            upload: Some(UploadResponse {
                upload_key: "main-key".to_owned(),
                ipv4: vec![Ipv4 {
                    external_address: u32::from_le_bytes([127, 0, 0, 1]),
                    external_port: 443,
                }],
                message_info,
                compatibility_message: vec![0x08, 0x01],
                sub_files: vec![SubFileInfo {
                    sub_file_type: 100,
                    upload_key: "thumb-key".to_owned(),
                    ipv4: vec![Ipv4 {
                        external_address: u32::from_le_bytes([127, 0, 0, 1]),
                        external_port: 443,
                    }],
                }],
            }),
        }
        .encode_to_vec();
        let outer = qq_wire::encode_oidb_request(0x11ea, 100, &rich, 0)?;
        let plan = parse_video_metadata_response(&outer, &MediaTarget::Group(42))?;
        assert_eq!(plan.uploads().len(), 2);
        assert_eq!(
            plan.uploads()
                .iter()
                .map(|upload| (upload.file_index(), upload.command_id()))
                .collect::<Vec<_>>(),
            [(0, 1_005), (1, 1_006)]
        );
        Ok(())
    }
}
