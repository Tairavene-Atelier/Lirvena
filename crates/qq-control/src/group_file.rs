use prost::Message;

use crate::{ControlError, ControlRequest, request_reserved};

const APP_ID: u32 = 7;
const BUS_ID: u32 = 102;
const MAX_FILE_FIELD_BYTES: usize = 4_096;

/// One frozen group-file mutation with its exact response binding.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroupFileControl {
    request: ControlRequest,
    response: GroupFileResponse,
}

impl GroupFileControl {
    /// Returns the transport request.
    #[must_use]
    pub const fn request(&self) -> &ControlRequest {
        &self.request
    }

    /// Validates both the outer OIDB result and the operation-specific result.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed, mismatched, missing, or rejected responses.
    pub fn parse_response(&self, input: &[u8]) -> Result<(), ControlError> {
        let outer = qq_wire::decode_oidb_response(input).map_err(|_error| ControlError)?;
        if outer.error_code() != 0 {
            return Err(ControlError);
        }
        let result = match self.response {
            GroupFileResponse::Delete => GroupFileResponseBody::decode(outer.body())?
                .delete
                .map(|result| result.code),
            GroupFileResponse::Move => GroupFileResponseBody::decode(outer.body())?
                .move_file
                .map(|result| result.code),
            GroupFileResponse::CreateFolder => GroupFolderResponseBody::decode(outer.body())?
                .create
                .map(|result| result.code),
            GroupFileResponse::DeleteFolder => GroupFolderResponseBody::decode(outer.body())?
                .delete
                .map(|result| result.code),
            GroupFileResponse::RenameFolder => GroupFolderResponseBody::decode(outer.body())?
                .rename
                .map(|result| result.code),
        };
        if result == Some(0) {
            Ok(())
        } else {
            Err(ControlError)
        }
    }
}

impl From<prost::DecodeError> for ControlError {
    fn from(_error: prost::DecodeError) -> Self {
        Self
    }
}

/// Encodes `delete_group_file`.
///
/// # Errors
///
/// Returns an error for a missing group or invalid file identifier.
pub fn delete_group_file(group_uin: u32, file_id: &str) -> Result<GroupFileControl, ControlError> {
    validate(group_uin, &[file_id])?;
    control(
        0x6d6,
        3,
        "OidbSvcTrpcTcp.0x6d6_3",
        GroupFileResponse::Delete,
        &GroupFileRequest {
            delete: Some(DeleteFile {
                group_uin,
                bus_id: BUS_ID,
                file_id: file_id.to_owned(),
            }),
            move_file: None,
        },
    )
}

/// Encodes `move_group_file`.
///
/// # Errors
///
/// Returns an error for a missing group or invalid file/directory identifier.
pub fn move_group_file(
    group_uin: u32,
    file_id: &str,
    parent_directory: &str,
    target_directory: &str,
) -> Result<GroupFileControl, ControlError> {
    validate(group_uin, &[file_id, parent_directory, target_directory])?;
    control(
        0x6d6,
        5,
        "OidbSvcTrpcTcp.0x6d6_5",
        GroupFileResponse::Move,
        &GroupFileRequest {
            delete: None,
            move_file: Some(MoveFile {
                group_uin,
                app_id: APP_ID,
                bus_id: BUS_ID,
                file_id: file_id.to_owned(),
                parent_directory: parent_directory.to_owned(),
                target_directory: target_directory.to_owned(),
            }),
        },
    )
}

/// Encodes `create_group_file_folder` at QQ's frozen root directory.
///
/// # Errors
///
/// Returns an error for a missing group or invalid folder name.
pub fn create_group_file_folder(
    group_uin: u32,
    name: &str,
) -> Result<GroupFileControl, ControlError> {
    validate(group_uin, &[name])?;
    folder_control(
        0,
        "OidbSvcTrpcTcp.0x6d7_0",
        GroupFileResponse::CreateFolder,
        &GroupFolderRequest {
            create: Some(CreateFolder {
                group_uin,
                root_directory: "/".to_owned(),
                name: name.to_owned(),
            }),
            delete: None,
            rename: None,
        },
    )
}

/// Encodes `delete_group_file_folder`.
///
/// # Errors
///
/// Returns an error for a missing group or invalid folder identifier.
pub fn delete_group_file_folder(
    group_uin: u32,
    folder_id: &str,
) -> Result<GroupFileControl, ControlError> {
    validate(group_uin, &[folder_id])?;
    folder_control(
        1,
        "OidbSvcTrpcTcp.0x6d7_1",
        GroupFileResponse::DeleteFolder,
        &GroupFolderRequest {
            create: None,
            delete: Some(DeleteFolder {
                group_uin,
                folder_id: folder_id.to_owned(),
            }),
            rename: None,
        },
    )
}

/// Encodes `rename_group_file_folder`.
///
/// # Errors
///
/// Returns an error for a missing group or invalid folder identifier/name.
pub fn rename_group_file_folder(
    group_uin: u32,
    folder_id: &str,
    new_name: &str,
) -> Result<GroupFileControl, ControlError> {
    validate(group_uin, &[folder_id, new_name])?;
    folder_control(
        2,
        "OidbSvcTrpcTcp.0x6d7_2",
        GroupFileResponse::RenameFolder,
        &GroupFolderRequest {
            create: None,
            delete: None,
            rename: Some(RenameFolder {
                group_uin,
                folder_id: folder_id.to_owned(),
                new_name: new_name.to_owned(),
            }),
        },
    )
}

fn folder_control(
    subcommand: u32,
    route: &'static str,
    response: GroupFileResponse,
    body: &GroupFolderRequest,
) -> Result<GroupFileControl, ControlError> {
    control(0x6d7, subcommand, route, response, body)
}

fn control(
    command: u32,
    subcommand: u32,
    route: &'static str,
    response: GroupFileResponse,
    body: &impl Message,
) -> Result<GroupFileControl, ControlError> {
    Ok(GroupFileControl {
        request: request_reserved(command, subcommand, route, None, 1, body)?,
        response,
    })
}

fn validate(group_uin: u32, fields: &[&str]) -> Result<(), ControlError> {
    if group_uin == 0
        || fields.iter().any(|value| {
            value.is_empty()
                || value.len() > MAX_FILE_FIELD_BYTES
                || value.chars().any(char::is_control)
        })
    {
        Err(ControlError)
    } else {
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum GroupFileResponse {
    Delete,
    Move,
    CreateFolder,
    DeleteFolder,
    RenameFolder,
}

#[derive(Clone, PartialEq, Message)]
struct GroupFileRequest {
    #[prost(message, optional, tag = "4")]
    delete: Option<DeleteFile>,
    #[prost(message, optional, tag = "6")]
    move_file: Option<MoveFile>,
}

#[derive(Clone, PartialEq, Message)]
struct DeleteFile {
    #[prost(uint32, tag = "1")]
    group_uin: u32,
    #[prost(uint32, tag = "3")]
    bus_id: u32,
    #[prost(string, tag = "5")]
    file_id: String,
}

#[derive(Clone, PartialEq, Message)]
struct MoveFile {
    #[prost(uint32, tag = "1")]
    group_uin: u32,
    #[prost(uint32, tag = "2")]
    app_id: u32,
    #[prost(uint32, tag = "3")]
    bus_id: u32,
    #[prost(string, tag = "4")]
    file_id: String,
    #[prost(string, tag = "5")]
    parent_directory: String,
    #[prost(string, tag = "6")]
    target_directory: String,
}

#[derive(Clone, PartialEq, Message)]
struct GroupFolderRequest {
    #[prost(message, optional, tag = "1")]
    create: Option<CreateFolder>,
    #[prost(message, optional, tag = "2")]
    delete: Option<DeleteFolder>,
    #[prost(message, optional, tag = "3")]
    rename: Option<RenameFolder>,
}

#[derive(Clone, PartialEq, Message)]
struct CreateFolder {
    #[prost(uint32, tag = "1")]
    group_uin: u32,
    #[prost(string, tag = "3")]
    root_directory: String,
    #[prost(string, tag = "4")]
    name: String,
}

#[derive(Clone, PartialEq, Message)]
struct DeleteFolder {
    #[prost(uint32, tag = "1")]
    group_uin: u32,
    #[prost(string, tag = "3")]
    folder_id: String,
}

#[derive(Clone, PartialEq, Message)]
struct RenameFolder {
    #[prost(uint32, tag = "1")]
    group_uin: u32,
    #[prost(string, tag = "3")]
    folder_id: String,
    #[prost(string, tag = "4")]
    new_name: String,
}

#[derive(Clone, PartialEq, Message)]
struct GroupFileResponseBody {
    #[prost(message, optional, tag = "4")]
    delete: Option<OperationResult>,
    #[prost(message, optional, tag = "6")]
    move_file: Option<OperationResult>,
}

#[derive(Clone, PartialEq, Message)]
struct GroupFolderResponseBody {
    #[prost(message, optional, tag = "1")]
    create: Option<OperationResult>,
    #[prost(message, optional, tag = "2")]
    delete: Option<OperationResult>,
    #[prost(message, optional, tag = "3")]
    rename: Option<OperationResult>,
}

#[derive(Clone, PartialEq, Message)]
struct OperationResult {
    #[prost(int32, tag = "1")]
    code: i32,
}

#[cfg(test)]
mod tests {
    use prost::Message;

    use super::*;

    #[test]
    fn mutation_requests_match_frozen_tags_and_constants() -> Result<(), Box<dyn std::error::Error>>
    {
        let delete = delete_group_file(100, "file")?;
        let outer = qq_wire::decode_oidb_request(delete.request().body())?;
        let body = GroupFileRequest::decode(outer.body())?;
        let value = body.delete.ok_or("delete missing")?;
        assert_eq!(
            (outer.command(), outer.subcommand(), outer.reserved()),
            (0x6d6, 3, 1)
        );
        assert_eq!((value.group_uin, value.bus_id), (100, BUS_ID));
        assert_eq!(value.file_id, "file");

        let create = create_group_file_folder(200, "folder")?;
        let outer = qq_wire::decode_oidb_request(create.request().body())?;
        let body = GroupFolderRequest::decode(outer.body())?;
        let value = body.create.ok_or("create missing")?;
        assert_eq!(
            (outer.command(), outer.subcommand(), outer.reserved()),
            (0x6d7, 0, 1)
        );
        assert_eq!(value.root_directory, "/");
        assert_eq!(value.name, "folder");
        Ok(())
    }

    #[test]
    fn inner_rejection_and_wrong_response_field_fail_closed()
    -> Result<(), Box<dyn std::error::Error>> {
        let control = move_group_file(100, "file", "/", "/target")?;
        let rejected = oidb_response(
            0,
            GroupFileResponseBody {
                delete: None,
                move_file: Some(OperationResult { code: 1 }),
            }
            .encode_to_vec(),
        );
        assert!(control.parse_response(&rejected).is_err());
        let wrong = oidb_response(
            0,
            GroupFileResponseBody {
                delete: Some(OperationResult { code: 0 }),
                move_file: None,
            }
            .encode_to_vec(),
        );
        assert!(control.parse_response(&wrong).is_err());
        Ok(())
    }

    #[test]
    fn unsafe_fields_fail_closed() {
        assert!(delete_group_file(0, "file").is_err());
        assert!(delete_group_file(1, "").is_err());
        assert!(rename_group_file_folder(1, "id", "bad\nname").is_err());
    }

    fn oidb_response(error_code: u32, body: Vec<u8>) -> Vec<u8> {
        TestOidbResponse { error_code, body }.encode_to_vec()
    }

    #[derive(Clone, PartialEq, Message)]
    struct TestOidbResponse {
        #[prost(uint32, tag = "3")]
        error_code: u32,
        #[prost(bytes = "vec", tag = "4")]
        body: Vec<u8>,
    }
}
