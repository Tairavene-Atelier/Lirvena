use prost::Message;
use qq_wire::decode_oidb_response;

use crate::highway_file::{HighwayFileExtensionSpec, encode_highway_file_extension};
use crate::{ControlError, ControlRequest, request, request_reserved};

use self::proto::{
    CompleteBody, CompleteEnvelope, CompleteInfo, UploadEnvelope, UploadRequest,
    UploadResponseEnvelope,
};

mod proto;
#[cfg(test)]
mod tests;

const MAX_TEXT_BYTES: usize = 4_096;
const MAX_OPAQUE_BYTES: usize = 64 * 1024;

/// Borrowed immutable metadata for one group-file upload application.
pub struct GroupFileUploadSpec<'a> {
    /// Numeric group identifier.
    pub group_uin: u32,
    /// Destination folder identifier, or `/` for the root.
    pub target_directory: &'a str,
    /// User-visible file name.
    pub file_name: &'a str,
    /// Complete object length.
    pub file_size: u64,
    /// SHA-1 of the complete object.
    pub sha1: &'a [u8; 20],
    /// MD5 of the complete object.
    pub md5: &'a [u8; 16],
}

/// Validated continuation returned by QQ for a group-file upload.
pub struct GroupFileUploadPlan {
    file_exists: bool,
    file_id: String,
    upload_key: Vec<u8>,
    check_key: Vec<u8>,
    upload_host: String,
    upload_port: u32,
}

impl GroupFileUploadPlan {
    /// Whether QQ already stores the complete object.
    #[must_use]
    pub const fn file_exists(&self) -> bool {
        self.file_exists
    }

    /// QQ file identifier used by the final completion request.
    #[must_use]
    pub fn file_id(&self) -> &str {
        &self.file_id
    }

    /// Encodes the audited Highway extension without exposing upload secrets in Debug output.
    ///
    /// # Errors
    ///
    /// Returns an error when caller identity or metadata is inconsistent with this plan.
    pub fn highway_extension(
        &self,
        sender_uin: u64,
        group_uin: u32,
        file_name: &str,
        file_size: u64,
        md5: &[u8; 16],
    ) -> Result<Vec<u8>, ControlError> {
        if self.file_exists
            || sender_uin == 0
            || group_uin == 0
            || file_size == 0
            || !valid_text(file_name)
        {
            return Err(ControlError);
        }
        let extension = encode_highway_file_extension(&HighwayFileExtensionSpec {
            sender_uin,
            receiver_uin: u64::from(group_uin),
            group_uin: u64::from(group_uin),
            file_size,
            md5,
            check_key: &self.check_key,
            file_id: &self.file_id,
            upload_key: &self.upload_key,
            file_name,
            upload_host: &self.upload_host,
            upload_port: self.upload_port,
            private_trailer: false,
        });
        if extension.is_empty() || extension.len() > MAX_OPAQUE_BYTES {
            return Err(ControlError);
        }
        Ok(extension)
    }
}

impl core::fmt::Debug for GroupFileUploadPlan {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("GroupFileUploadPlan")
            .field("file_exists", &self.file_exists)
            .field("file_id_len", &self.file_id.len())
            .finish_non_exhaustive()
    }
}

/// Encodes the audited 52194 group-file upload application.
///
/// # Errors
///
/// Returns an error for empty objects, invalid identifiers or unbounded text.
pub fn group_file_upload_request(
    spec: &GroupFileUploadSpec<'_>,
) -> Result<ControlRequest, ControlError> {
    if spec.group_uin == 0
        || spec.file_size == 0
        || !valid_text(spec.target_directory)
        || !valid_text(spec.file_name)
    {
        return Err(ControlError);
    }
    request_reserved(
        0x06d6,
        0,
        "OidbSvcTrpcTcp.0x6d6_0",
        None,
        1,
        &UploadEnvelope {
            upload: Some(UploadRequest {
                group_uin: spec.group_uin,
                app_id: 7,
                business_id: 102,
                entrance: 6,
                target_directory: spec.target_directory.to_owned(),
                file_name: spec.file_name.to_owned(),
                local_path: format!("/{}", spec.file_name),
                file_size: spec.file_size,
                sha1: spec.sha1.to_vec(),
                sha3: Vec::new(),
                md5: spec.md5.to_vec(),
                field15: true,
            }),
        },
    )
}

/// Parses and bounds one upload application response.
///
/// # Errors
///
/// Returns an error for malformed, rejected or unusable continuations.
pub fn parse_group_file_upload_response(input: &[u8]) -> Result<GroupFileUploadPlan, ControlError> {
    let response = decode_oidb_response(input).map_err(|_error| ControlError)?;
    if response.error_code() != 0 {
        return Err(ControlError);
    }
    let upload = UploadResponseEnvelope::decode(response.body())
        .map_err(|_error| ControlError)?
        .upload
        .ok_or(ControlError)?;
    if upload.result != 0
        || !valid_text(&upload.file_id)
        || upload.upload_key.len() > MAX_OPAQUE_BYTES
        || upload.check_key.len() > MAX_OPAQUE_BYTES
        || upload.upload_host.len() > MAX_TEXT_BYTES
    {
        return Err(ControlError);
    }
    if !upload.file_exists
        && (upload.upload_key.is_empty()
            || upload.check_key.is_empty()
            || !valid_text(&upload.upload_host)
            || upload.upload_port == 0)
    {
        return Err(ControlError);
    }
    Ok(GroupFileUploadPlan {
        file_exists: upload.file_exists,
        file_id: upload.file_id,
        upload_key: upload.upload_key,
        check_key: upload.check_key,
        upload_host: upload.upload_host,
        upload_port: upload.upload_port,
    })
}

/// Encodes the signed completion step after fast-upload or Highway transfer.
///
/// # Errors
///
/// Returns an error for invalid identifiers.
pub fn group_file_complete_request(
    group_uin: u32,
    file_id: &str,
    random: u32,
) -> Result<ControlRequest, ControlError> {
    if group_uin == 0 || !valid_text(file_id) {
        return Err(ControlError);
    }
    request(
        0x06d9,
        4,
        "OidbSvcTrpcTcp.0x6d9_4",
        Some(11),
        &CompleteEnvelope {
            body: Some(CompleteBody {
                group_uin,
                kind: 2,
                info: Some(CompleteInfo {
                    business_type: 102,
                    file_id: file_id.to_owned(),
                    random,
                    field5: true,
                }),
            }),
        },
    )
}

fn valid_text(value: &str) -> bool {
    !value.is_empty() && value.len() <= MAX_TEXT_BYTES && !value.chars().any(char::is_control)
}
