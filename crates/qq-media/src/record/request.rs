use super::{RecordDescriptor, RecordMetadataRequest};
use crate::image::upper_hex;
use crate::image_proto::{FileInfo, FileType, PictureBusiness, VideoBusiness, VoiceBusiness};
use crate::rich_request::{RichRequestSpec, encode};
use crate::{MediaError, MediaTarget};

const DIRECT_RESERVE: &[u8] = &[0x08, 0x00, 0x38, 0x00];
const DIRECT_GENERAL_FLAGS: &[u8] = &[
    0x9a, 0x01, 0x0b, 0xaa, 0x03, 0x08, 0x08, 0x04, 0x12, 0x04, 0x00, 0x00, 0x00, 0x00,
];
const GROUP_PROTOBUF_RESERVE: &[u8] = &[0x08, 0x00, 0x38, 0x00];
const GROUP_GENERAL_FLAGS: &[u8] = &[0x9a, 0x01, 0x07, 0xaa, 0x03, 0x04, 0x08, 0x08, 0x12, 0x00];

/// Encodes the audited private or group record metadata request.
///
/// # Errors
///
/// Returns an error for an invalid target or zero client random value.
pub fn encode_record_metadata_request(
    target: MediaTarget<'_>,
    record: &RecordDescriptor,
    client_random_id: u32,
) -> Result<RecordMetadataRequest, MediaError> {
    let md5 = upper_hex(&record.md5);
    let encoded = encode(RichRequestSpec {
        target,
        client_random_id,
        direct_route: ("OidbSvcTrpcTcp.0x126d_100", 0x126d),
        group_route: ("OidbSvcTrpcTcp.0x126e_100", 0x126e),
        business_type: 3,
        file: FileInfo {
            size: record.size,
            md5: md5.clone(),
            sha1: upper_hex(&record.sha1),
            name: format!("{md5}{}", record.format.extension()),
            kind: Some(FileType {
                kind: 3,
                picture_format: 0,
                video_format: 0,
                voice_format: 1,
            }),
            width: 0,
            height: 0,
            duration: record.duration_seconds,
            original: 0,
        },
        picture: PictureBusiness {
            business_type: 0,
            summary: String::new(),
            reserve: Vec::new(),
        },
        video: VideoBusiness {
            reserve: Vec::new(),
        },
        direct_voice: VoiceBusiness {
            reserve: DIRECT_RESERVE.to_vec(),
            protobuf_reserve: Vec::new(),
            general_flags: DIRECT_GENERAL_FLAGS.to_vec(),
        },
        group_voice: VoiceBusiness {
            reserve: Vec::new(),
            protobuf_reserve: GROUP_PROTOBUF_RESERVE.to_vec(),
            general_flags: GROUP_GENERAL_FLAGS.to_vec(),
        },
    })?;
    Ok(RecordMetadataRequest {
        command: encoded.command,
        body: encoded.body,
    })
}

#[cfg(test)]
mod tests {
    use prost::Message;

    use super::encode_record_metadata_request;
    use crate::image_proto::RichRequest;
    use crate::{MediaTarget, RecordDescriptor, RecordFormat};

    #[test]
    fn direct_request_matches_audited_voice_fields() -> Result<(), Box<dyn std::error::Error>> {
        let request = encode_record_metadata_request(
            MediaTarget::Direct("u_target"),
            &RecordDescriptor {
                size: 123,
                duration_seconds: 4,
                format: RecordFormat::TencentSilkV3,
                md5: [0xab; 16],
                sha1: [0xcd; 20],
            },
            7,
        )?;
        assert_eq!(request.command(), "OidbSvcTrpcTcp.0x126d_100");
        let outer = qq_wire::decode_oidb_request(request.body())?;
        assert_eq!((outer.command(), outer.subcommand()), (0x126d, 100));
        let rich = RichRequest::decode(outer.body())?;
        let head = rich.head.ok_or(crate::MediaError::ReferenceRejected)?;
        let scene = head.scene.ok_or(crate::MediaError::ReferenceRejected)?;
        assert_eq!((scene.kind, scene.business_type), (1, 3));
        let upload = rich.upload.ok_or(crate::MediaError::ReferenceRejected)?;
        let file = upload
            .files
            .first()
            .and_then(|value| value.file.as_ref())
            .ok_or(crate::MediaError::ReferenceRejected)?;
        assert_eq!((file.size, file.duration), (123, 4));
        assert_eq!(file.kind.as_ref().map(|kind| kind.voice_format), Some(1));
        let voice = upload
            .business
            .and_then(|value| value.voice)
            .ok_or(crate::MediaError::ReferenceRejected)?;
        assert_eq!(voice.reserve, [0x08, 0x00, 0x38, 0x00]);
        assert!(!voice.general_flags.is_empty());
        Ok(())
    }
}
