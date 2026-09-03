use super::{ImageDescriptor, ImageMetadataRequest, upper_hex};
use crate::image_proto::{FileInfo, FileType, PictureBusiness, VideoBusiness, VoiceBusiness};
use crate::rich_request::{RichRequestSpec, encode};
use crate::{MediaError, MediaTarget};

const DIRECT_RESERVE: &[u8] = &[
    0x08, 0x00, 0x18, 0x00, 0x20, 0x00, 0x42, 0x00, 0x50, 0x00, 0x62, 0x00, 0x92, 0x01, 0x00, 0x9a,
    0x01, 0x00, 0xa2, 0x01, 0x0c, 0x08, 0x00, 0x12, 0x00, 0x18, 0x00, 0x20, 0x00, 0x28, 0x00, 0x3a,
    0x00,
];
const GROUP_RESERVE: &[u8] = &[
    0x08, 0x00, 0x18, 0x00, 0x20, 0x00, 0x4a, 0x00, 0x50, 0x00, 0x62, 0x00, 0x92, 0x01, 0x00, 0x9a,
    0x01, 0x00, 0xaa, 0x01, 0x0c, 0x08, 0x00, 0x12, 0x00, 0x18, 0x00, 0x20, 0x00, 0x28, 0x00, 0x3a,
    0x00,
];

/// Encodes the audited private or group image metadata request.
///
/// # Errors
///
/// Returns an error for an invalid target or zero client random value.
pub fn encode_image_metadata_request(
    target: MediaTarget<'_>,
    image: &ImageDescriptor,
    client_random_id: u32,
) -> Result<ImageMetadataRequest, MediaError> {
    let md5 = upper_hex(&image.md5);
    let reserve = match target {
        MediaTarget::Direct(_) => DIRECT_RESERVE,
        MediaTarget::Group(_) => GROUP_RESERVE,
    };
    let encoded = encode(RichRequestSpec {
        target,
        client_random_id,
        direct_route: ("OidbSvcTrpcTcp.0x11c5_100", 0x11c5),
        group_route: ("OidbSvcTrpcTcp.0x11c4_100", 0x11c4),
        business_type: 1,
        file: FileInfo {
            size: image.size,
            md5: md5.clone(),
            sha1: upper_hex(&image.sha1),
            name: format!("{md5}{}", image.format.extension()),
            kind: Some(FileType {
                kind: 1,
                picture_format: image.format.qq_code(),
                video_format: 0,
                voice_format: 0,
            }),
            width: image.width,
            height: image.height,
            duration: 0,
            original: 1,
        },
        picture: PictureBusiness {
            business_type: 0,
            summary: String::new(),
            reserve: reserve.to_vec(),
        },
        video: VideoBusiness {
            reserve: Vec::new(),
        },
        direct_voice: empty_voice(),
        group_voice: empty_voice(),
    })?;
    Ok(ImageMetadataRequest {
        command: encoded.command,
        body: encoded.body,
    })
}

fn empty_voice() -> VoiceBusiness {
    VoiceBusiness {
        reserve: Vec::new(),
        protobuf_reserve: Vec::new(),
        general_flags: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use prost::Message;

    use super::encode_image_metadata_request;
    use crate::MediaTarget;
    use crate::image::{ImageDescriptor, ImageFormat};
    use crate::image_proto::RichRequest;

    #[test]
    fn group_request_matches_audited_scene_and_file_fields()
    -> Result<(), Box<dyn std::error::Error>> {
        let request = encode_image_metadata_request(
            MediaTarget::Group(42),
            &ImageDescriptor {
                size: 123,
                width: 10,
                height: 20,
                format: ImageFormat::Png,
                md5: [0xab; 16],
                sha1: [0xcd; 20],
            },
            7,
        )?;
        assert_eq!(request.command(), "OidbSvcTrpcTcp.0x11c4_100");
        let outer = qq_wire::decode_oidb_request(request.body())?;
        assert_eq!(
            (outer.command(), outer.subcommand(), outer.reserved()),
            (0x11c4, 100, 1)
        );
        let rich = RichRequest::decode(outer.body())?;
        let head = rich.head.ok_or(crate::MediaError::ReferenceRejected)?;
        let scene = head.scene.ok_or(crate::MediaError::ReferenceRejected)?;
        assert_eq!(
            (scene.request_type, scene.business_type, scene.kind),
            (2, 1, 2)
        );
        assert_eq!(scene.group.map(|group| group.group_code), Some(42));
        let upload = rich.upload.ok_or(crate::MediaError::ReferenceRejected)?;
        let file = upload
            .files
            .first()
            .and_then(|value| value.file.as_ref())
            .ok_or(crate::MediaError::ReferenceRejected)?;
        assert_eq!((file.size, file.width, file.height), (123, 10, 20));
        assert_eq!(file.name, format!("{}{}.png", "AB".repeat(15), "AB"));
        assert_eq!(
            file.kind.as_ref().map(|kind| kind.picture_format),
            Some(1_001)
        );
        assert!(upload.try_fast_upload);
        assert_eq!(
            (
                upload.client_random_id,
                upload.compatibility_scene,
                upload.client_sequence
            ),
            (7, 2, 10)
        );
        Ok(())
    }
}
