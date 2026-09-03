use super::{VideoDescriptor, VideoMetadataRequest};
use crate::image::{ImageDescriptor, upper_hex};
use crate::image_proto::{
    FileInfo, FileType, PictureBusiness, UploadInfo, VideoBusiness, VoiceBusiness,
};
use crate::rich_request::{RichRequestSpec, encode};
use crate::{MediaError, MediaTarget};

const VIDEO_RESERVE: &[u8] = &[0x80, 0x01, 0x00];

/// Encodes the audited private or group video metadata request with one thumbnail sub-file.
///
/// # Errors
///
/// Returns an error for an invalid target or zero client random value.
pub fn encode_video_metadata_request(
    target: MediaTarget<'_>,
    video: &VideoDescriptor,
    thumbnail: &ImageDescriptor,
    client_random_id: u32,
) -> Result<VideoMetadataRequest, MediaError> {
    let video_md5 = upper_hex(&video.md5());
    let thumbnail_md5 = upper_hex(&thumbnail.md5());
    let empty_voice = || VoiceBusiness {
        reserve: Vec::new(),
        protobuf_reserve: Vec::new(),
        general_flags: Vec::new(),
    };
    let encoded = encode(RichRequestSpec {
        target,
        client_random_id,
        direct_route: ("OidbSvcTrpcTcp.0x11e9_100", 0x11e9),
        group_route: ("OidbSvcTrpcTcp.0x11ea_100", 0x11ea),
        business_type: 2,
        files: vec![
            UploadInfo {
                file: Some(FileInfo {
                    size: video.size(),
                    md5: video_md5.clone(),
                    sha1: upper_hex(&video.sha1()),
                    name: format!("{video_md5}.mp4"),
                    kind: Some(FileType {
                        kind: 2,
                        picture_format: 0,
                        video_format: 0,
                        voice_format: 0,
                    }),
                    width: 0,
                    height: 0,
                    duration: 0,
                    original: 0,
                }),
                sub_file_type: 0,
            },
            UploadInfo {
                file: Some(FileInfo {
                    size: thumbnail.size(),
                    md5: thumbnail_md5.clone(),
                    sha1: upper_hex(&thumbnail.sha1()),
                    name: format!("{thumbnail_md5}.png"),
                    kind: Some(FileType {
                        kind: 1,
                        picture_format: 1_001,
                        video_format: 0,
                        voice_format: 0,
                    }),
                    width: thumbnail.width(),
                    height: thumbnail.height(),
                    duration: 0,
                    original: 0,
                }),
                sub_file_type: 100,
            },
        ],
        picture: PictureBusiness {
            business_type: 0,
            summary: String::new(),
            reserve: Vec::new(),
        },
        video: VideoBusiness {
            reserve: VIDEO_RESERVE.to_vec(),
        },
        direct_voice: empty_voice(),
        group_voice: empty_voice(),
    })?;
    Ok(VideoMetadataRequest {
        command: encoded.command,
        body: encoded.body,
    })
}

#[cfg(test)]
mod tests {
    use prost::Message;

    use super::encode_video_metadata_request;
    use crate::image_proto::RichRequest;
    use crate::{MediaTarget, analyze_image, analyze_video, default_video_thumbnail};

    #[test]
    fn group_request_contains_main_video_and_thumbnail() -> Result<(), Box<dyn std::error::Error>> {
        let video = analyze_video(&[0, 0, 0, 12, b'f', b't', b'y', b'p', b'i', b's', b'o', b'm'])?;
        let thumbnail = analyze_image(default_video_thumbnail())?;
        let request = encode_video_metadata_request(MediaTarget::Group(42), &video, &thumbnail, 7)?;
        assert_eq!(request.command(), "OidbSvcTrpcTcp.0x11ea_100");
        let outer = qq_wire::decode_oidb_request(request.body())?;
        assert_eq!((outer.command(), outer.subcommand()), (0x11ea, 100));
        let rich = RichRequest::decode(outer.body())?;
        let upload = rich.upload.ok_or(crate::MediaError::ReferenceRejected)?;
        assert_eq!(upload.files.len(), 2);
        assert_eq!(upload.files[0].sub_file_type, 0);
        assert_eq!(upload.files[1].sub_file_type, 100);
        assert_eq!(
            upload.files[1]
                .file
                .as_ref()
                .and_then(|file| file.kind.as_ref())
                .map(|kind| (kind.kind, kind.picture_format)),
            Some((1, 1_001))
        );
        assert_eq!(
            upload
                .business
                .and_then(|business| business.video)
                .map(|video| video.reserve),
            Some(vec![0x80, 0x01, 0x00])
        );
        Ok(())
    }
}
