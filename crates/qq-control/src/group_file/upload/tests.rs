use prost::Message;
use qq_wire::{decode_oidb_request, encode_oidb_request};

use super::proto::UploadResponse;
use super::*;

#[test]
fn upload_and_completion_preserve_frozen_constants() -> Result<(), Box<dyn std::error::Error>> {
    let request = group_file_upload_request(&GroupFileUploadSpec {
        group_uin: 42,
        target_directory: "/folder",
        file_name: "a.bin",
        file_size: 3,
        sha1: &[0x11; 20],
        md5: &[0x22; 16],
    })?;
    let outer = decode_oidb_request(request.body())?;
    assert_eq!((outer.command(), outer.subcommand()), (0x6d6, 0));
    assert_eq!(outer.reserved(), 1);
    let upload = UploadEnvelope::decode(outer.body())?
        .upload
        .ok_or(ControlError)?;
    assert_eq!(
        (upload.app_id, upload.business_id, upload.entrance),
        (7, 102, 6)
    );
    assert_eq!(upload.local_path, "/a.bin");
    assert!(upload.field15);

    let complete = group_file_complete_request(42, "file-id", 9)?;
    assert_eq!(complete.signing_operation(), Some(11));
    let outer = decode_oidb_request(complete.body())?;
    assert_eq!(outer.reserved(), 0);
    let body = CompleteEnvelope::decode(outer.body())?
        .body
        .ok_or(ControlError)?;
    let info = body.info.ok_or(ControlError)?;
    assert_eq!((body.kind, info.business_type, info.random), (2, 102, 9));
    assert!(info.field5);
    Ok(())
}

#[test]
fn response_builds_bounded_highway_extension() -> Result<(), Box<dyn std::error::Error>> {
    let inner = UploadResponseEnvelope {
        upload: Some(UploadResponse {
            result: 0,
            upload_host: "1.2.3.4".to_owned(),
            file_id: "file-id".to_owned(),
            check_key: vec![1, 2],
            upload_key: vec![3, 4],
            file_exists: false,
            upload_port: 443,
        }),
    }
    .encode_to_vec();
    let response = encode_oidb_request(0x6d6, 0, &inner, 0)?;
    let plan = parse_group_file_upload_response(&response)?;
    let extension = plan.highway_extension(10001, 42, "a.bin", 3, &[5; 16])?;
    let decoded = HighwayExtension::decode(extension.as_slice())?;
    let entry = decoded.entry.ok_or(ControlError)?;
    assert_eq!(entry.business.ok_or(ControlError)?.group, 42);
    assert_eq!(entry.file.ok_or(ControlError)?.check_key, vec![1, 2]);
    Ok(())
}

#[test]
fn malformed_continuations_fail_closed() -> Result<(), Box<dyn std::error::Error>> {
    let inner = UploadResponseEnvelope {
        upload: Some(UploadResponse {
            result: 0,
            upload_host: String::new(),
            file_id: "file-id".to_owned(),
            check_key: Vec::new(),
            upload_key: Vec::new(),
            file_exists: false,
            upload_port: 0,
        }),
    }
    .encode_to_vec();
    let response = encode_oidb_request(0x6d6, 0, &inner, 0)?;
    assert!(parse_group_file_upload_response(&response).is_err());
    Ok(())
}
