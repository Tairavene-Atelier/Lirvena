use prost::Message;

use crate::highway_file::{HighwayFileExtensionSpec, encode_highway_file_extension};
use crate::{ControlError, ControlRequest, request, validate_uid};

const MAX_TEXT_BYTES: usize = 4 * 1024;
const MAX_OPAQUE_BYTES: usize = 64 * 1024;

/// Borrowed metadata for one private-file upload application.
pub struct PrivateFileUploadSpec<'a> {
    /// Current account UID.
    pub sender_uid: &'a str,
    /// Current recipient UID resolved from the friend directory.
    pub receiver_uid: &'a str,
    /// User-visible file name.
    pub file_name: &'a str,
    /// Complete object size.
    pub file_size: u32,
    /// MD5 of at most the first ten MiB.
    pub prefix_md5: &'a [u8; 16],
    /// SHA-1 of the complete object.
    pub sha1: &'a [u8; 20],
    /// MD5 of the complete object.
    pub md5: &'a [u8; 16],
}

/// Validated continuation returned by QQ for a private-file upload.
pub struct PrivateFileUploadPlan {
    file_exists: bool,
    file_id: String,
    upload_key: Vec<u8>,
    upload_host: String,
    upload_port: u32,
    file_hash: String,
}

impl PrivateFileUploadPlan {
    /// Whether QQ already stores the complete object.
    #[must_use]
    pub const fn file_exists(&self) -> bool {
        self.file_exists
    }

    /// File identifier used in the final private message.
    #[must_use]
    pub fn file_id(&self) -> &str {
        &self.file_id
    }

    /// File hash/addon used in the final private message.
    #[must_use]
    pub fn file_hash(&self) -> &str {
        &self.file_hash
    }

    /// Encodes the shared QQ file Highway extension for private command 95.
    ///
    /// # Errors
    ///
    /// Returns an error for inconsistent identity or metadata.
    pub fn highway_extension(
        &self,
        sender_uin: u64,
        file_name: &str,
        file_size: u64,
        md5: &[u8; 16],
        sha1: &[u8; 20],
    ) -> Result<Vec<u8>, ControlError> {
        if self.file_exists
            || sender_uin == 0
            || file_size == 0
            || !valid_text(file_name)
            || self.upload_key.is_empty()
            || self.upload_host.is_empty()
            || self.upload_port == 0
        {
            return Err(ControlError);
        }
        let extension = encode_highway_file_extension(&HighwayFileExtensionSpec {
            sender_uin,
            receiver_uin: 0,
            group_uin: 0,
            file_size,
            md5,
            check_key: sha1,
            file_id: &self.file_id,
            upload_key: &self.upload_key,
            file_name,
            upload_host: &self.upload_host,
            upload_port: self.upload_port,
            private_trailer: true,
        });
        if extension.is_empty() || extension.len() > MAX_OPAQUE_BYTES {
            return Err(ControlError);
        }
        Ok(extension)
    }
}

impl core::fmt::Debug for PrivateFileUploadPlan {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("PrivateFileUploadPlan")
            .field("file_exists", &self.file_exists)
            .field("file_id_len", &self.file_id.len())
            .finish_non_exhaustive()
    }
}

/// Encodes the frozen Linux private-file upload application.
///
/// # Errors
///
/// Returns an error for invalid identities, names, sizes, or checksums.
pub fn private_file_upload_request(
    spec: &PrivateFileUploadSpec<'_>,
) -> Result<ControlRequest, ControlError> {
    validate_uid(spec.sender_uid)?;
    validate_uid(spec.receiver_uid)?;
    if spec.file_size == 0 || !valid_text(spec.file_name) {
        return Err(ControlError);
    }
    request(
        0x0e37,
        1700,
        "OidbSvcTrpcTcp.0xe37_1700",
        None,
        &UploadEnvelope {
            command: 1700,
            sequence: 0,
            upload: Some(UploadRequest {
                sender_uid: spec.sender_uid.to_owned(),
                receiver_uid: spec.receiver_uid.to_owned(),
                file_size: spec.file_size,
                file_name: spec.file_name.to_owned(),
                prefix_md5: spec.prefix_md5.to_vec(),
                sha1: spec.sha1.to_vec(),
                local_path: "/".to_owned(),
                md5: spec.md5.to_vec(),
                sha3: Vec::new(),
            }),
            business_id: 3,
            client_type: 1,
            media_platform: 1,
        },
    )
}

/// Parses one private-file upload application response.
///
/// # Errors
///
/// Returns an error for rejected, malformed, incomplete, or excessive continuations.
pub fn parse_private_file_upload_response(
    input: &[u8],
) -> Result<PrivateFileUploadPlan, ControlError> {
    let outer = qq_wire::decode_oidb_response(input).map_err(|_error| ControlError)?;
    if outer.error_code() != 0 {
        return Err(ControlError);
    }
    let response = UploadEnvelopeResponse::decode(outer.body()).map_err(|_error| ControlError)?;
    let upload = response.upload.ok_or(ControlError)?;
    if upload.result != 0
        || !valid_text(&upload.file_id)
        || !valid_text(&upload.file_hash)
        || upload.media_upload_key.len() > MAX_OPAQUE_BYTES
        || upload.upload_host.len() > MAX_TEXT_BYTES
    {
        return Err(ControlError);
    }
    if !upload.file_exists
        && (upload.media_upload_key.is_empty()
            || !valid_text(&upload.upload_host)
            || upload.upload_port == 0)
    {
        return Err(ControlError);
    }
    Ok(PrivateFileUploadPlan {
        file_exists: upload.file_exists,
        file_id: upload.file_id,
        upload_key: upload.media_upload_key,
        upload_host: upload.upload_host,
        upload_port: upload.upload_port,
        file_hash: upload.file_hash,
    })
}

fn valid_text(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_TEXT_BYTES
        && value.trim() == value
        && !value.chars().any(char::is_control)
}

#[derive(Clone, PartialEq, Message)]
struct UploadEnvelope {
    #[prost(uint32, tag = "1")]
    command: u32,
    #[prost(int32, tag = "2")]
    sequence: i32,
    #[prost(message, optional, tag = "19")]
    upload: Option<UploadRequest>,
    #[prost(int32, tag = "101")]
    business_id: i32,
    #[prost(int32, tag = "102")]
    client_type: i32,
    #[prost(int32, tag = "200")]
    media_platform: i32,
}

#[derive(Clone, PartialEq, Message)]
struct UploadRequest {
    #[prost(string, tag = "10")]
    sender_uid: String,
    #[prost(string, tag = "20")]
    receiver_uid: String,
    #[prost(uint32, tag = "30")]
    file_size: u32,
    #[prost(string, tag = "40")]
    file_name: String,
    #[prost(bytes = "vec", tag = "50")]
    prefix_md5: Vec<u8>,
    #[prost(bytes = "vec", tag = "60")]
    sha1: Vec<u8>,
    #[prost(string, tag = "70")]
    local_path: String,
    #[prost(bytes = "vec", tag = "110")]
    md5: Vec<u8>,
    #[prost(bytes = "vec", tag = "120")]
    sha3: Vec<u8>,
}

#[derive(Clone, PartialEq, Message)]
struct UploadEnvelopeResponse {
    #[prost(message, optional, tag = "19")]
    upload: Option<UploadResponse>,
}

#[derive(Clone, PartialEq, Message)]
struct UploadResponse {
    #[prost(int32, tag = "10")]
    result: i32,
    #[prost(string, tag = "60")]
    upload_host: String,
    #[prost(uint32, tag = "80")]
    upload_port: u32,
    #[prost(string, tag = "90")]
    file_id: String,
    #[prost(bool, tag = "110")]
    file_exists: bool,
    #[prost(string, tag = "200")]
    file_hash: String,
    #[prost(bytes = "vec", tag = "220")]
    media_upload_key: Vec<u8>,
}

#[cfg(test)]
mod tests {
    use prost::Message;

    use super::*;
    use crate::highway_file::HighwayExtension;

    #[test]
    fn request_preserves_frozen_upload_fields() -> Result<(), Box<dyn std::error::Error>> {
        let request = private_file_upload_request(&PrivateFileUploadSpec {
            sender_uid: "u_self",
            receiver_uid: "u_peer",
            file_name: "a.bin",
            file_size: 3,
            prefix_md5: &[1; 16],
            sha1: &[2; 20],
            md5: &[3; 16],
        })?;
        assert_eq!(request.command(), "OidbSvcTrpcTcp.0xe37_1700");
        assert_eq!(request.signing_operation(), None);
        let outer = qq_wire::decode_oidb_request(request.body())?;
        assert_eq!(
            (outer.command(), outer.subcommand(), outer.reserved()),
            (0x0e37, 1700, 0)
        );
        let request = UploadEnvelope::decode(outer.body())?;
        assert_eq!((request.command, request.sequence), (1700, 0));
        assert_eq!(
            (
                request.business_id,
                request.client_type,
                request.media_platform
            ),
            (3, 1, 1)
        );
        let upload = request.upload.ok_or(ControlError)?;
        assert_eq!(
            (upload.sender_uid.as_str(), upload.receiver_uid.as_str()),
            ("u_self", "u_peer")
        );
        assert_eq!((upload.file_size, upload.local_path.as_str()), (3, "/"));
        assert_eq!(
            (upload.prefix_md5.len(), upload.sha1.len(), upload.md5.len()),
            (16, 20, 16)
        );
        Ok(())
    }

    #[test]
    fn response_builds_private_highway_extension() -> Result<(), Box<dyn std::error::Error>> {
        let inner = UploadEnvelopeResponse {
            upload: Some(UploadResponse {
                result: 0,
                upload_host: "1.2.3.4".to_owned(),
                upload_port: 443,
                file_id: "file-id".to_owned(),
                file_exists: false,
                file_hash: "addon".to_owned(),
                media_upload_key: vec![1, 2],
            }),
        }
        .encode_to_vec();
        let outer = qq_wire::encode_oidb_request(0x0e37, 1700, &inner, 0)?;
        let plan = parse_private_file_upload_response(&outer)?;
        let extension = plan.highway_extension(10001, "a.bin", 3, &[3; 16], &[2; 20])?;
        let extension = HighwayExtension::decode(extension.as_slice())?;
        assert_eq!(extension.private_trailer, 1);
        let entry = extension.entry.ok_or(ControlError)?;
        let business = entry.business.ok_or(ControlError)?;
        assert_eq!(
            (business.sender, business.receiver, business.group),
            (10001, 0, 0)
        );
        assert_eq!(entry.file.ok_or(ControlError)?.check_key, [2; 20]);
        Ok(())
    }

    #[test]
    fn incomplete_or_rejected_plans_fail_closed() -> Result<(), Box<dyn std::error::Error>> {
        let inner = UploadEnvelopeResponse {
            upload: Some(UploadResponse {
                result: 0,
                upload_host: String::new(),
                upload_port: 0,
                file_id: "file-id".to_owned(),
                file_exists: false,
                file_hash: "addon".to_owned(),
                media_upload_key: Vec::new(),
            }),
        }
        .encode_to_vec();
        let outer = qq_wire::encode_oidb_request(0x0e37, 1700, &inner, 0)?;
        assert!(parse_private_file_upload_response(&outer).is_err());
        Ok(())
    }
}
