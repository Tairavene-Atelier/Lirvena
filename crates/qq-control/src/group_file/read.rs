use core::fmt::Write as _;
use prost::Message;

use crate::{ControlError, ControlRequest, request_reserved};

const MAX_TEXT_BYTES: usize = 4_096;
const MAX_PAGE_ITEMS: usize = 20;

/// One decoded QQ group-file entry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GroupFileEntry {
    /// A regular file.
    File(GroupFileInfo),
    /// A directory.
    Folder(GroupFolderInfo),
}

/// Public OneBot-facing fields from one QQ group file.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroupFileInfo {
    /// QQ file identifier.
    pub file_id: String,
    /// Display name.
    pub file_name: String,
    /// File length in bytes.
    pub file_size: u64,
    /// QQ business identifier.
    pub bus_id: u32,
    /// Upload Unix timestamp.
    pub uploaded_time: u32,
    /// Expiry Unix timestamp.
    pub expire_time: u32,
    /// Last-modified Unix timestamp.
    pub modified_time: u32,
    /// QQ-reported download count.
    pub downloaded_times: u32,
    /// Numeric uploader QQ identifier.
    pub uploader_uin: u32,
    /// QQ-reported uploader display name.
    pub uploader_name: String,
}

/// Public OneBot-facing fields from one QQ group folder.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroupFolderInfo {
    /// QQ folder identifier.
    pub folder_id: String,
    /// Display name.
    pub folder_name: String,
    /// Creation Unix timestamp.
    pub create_time: u32,
    /// Last-modified Unix timestamp.
    pub modified_time: u32,
    /// Numeric creator QQ identifier.
    pub creator_uin: u32,
    /// QQ-reported creator display name.
    pub creator_name: String,
    /// Number of direct files reported by QQ.
    pub total_file_count: u32,
}

/// One bounded group-file list page.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroupFilePage {
    /// Validated entries in this page.
    pub entries: Vec<GroupFileEntry>,
    /// Whether QQ marks this as the final page.
    pub is_end: bool,
}

/// Encodes one frozen 20-entry group-file page request.
///
/// # Errors
///
/// Returns an error for a missing group, invalid directory, or overflowing start index.
pub fn group_file_list_request(
    group_uin: u32,
    directory: &str,
    start_index: u32,
) -> Result<ControlRequest, ControlError> {
    validate(group_uin, directory)?;
    request_reserved(
        0x6d8,
        1,
        "OidbSvcTrpcTcp.0x6d8_1",
        None,
        1,
        &ViewRequest {
            list: Some(ListRequest {
                group_uin,
                app_id: 7,
                directory: directory.to_owned(),
                count: 20,
                sort_by: 1,
                start_index,
                field_seventeen: 2,
                field_eighteen: 0,
            }),
        },
    )
}

/// Decodes one exact group-file page and rejects unknown entry kinds.
///
/// # Errors
///
/// Returns an error for outer/inner rejection, malformed data, or excessive/ambiguous entries.
pub fn parse_group_file_list_response(input: &[u8]) -> Result<GroupFilePage, ControlError> {
    let outer = qq_wire::decode_oidb_response(input).map_err(|_error| ControlError)?;
    if outer.error_code() != 0 {
        return Err(ControlError);
    }
    let list = ViewResponse::decode(outer.body())?
        .list
        .ok_or(ControlError)?;
    if list.code != 0 || list.items.len() > MAX_PAGE_ITEMS {
        return Err(ControlError);
    }
    let entries = list
        .items
        .into_iter()
        .map(decode_entry)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(GroupFilePage {
        entries,
        is_end: list.is_end,
    })
}

/// Encodes `get_group_file_url`.
///
/// # Errors
///
/// Returns an error for a missing group or invalid file identifier.
pub fn group_file_download_request(
    group_uin: u32,
    file_id: &str,
) -> Result<ControlRequest, ControlError> {
    validate(group_uin, file_id)?;
    if !valid_file_id(file_id) {
        return Err(ControlError);
    }
    request_reserved(
        0x6d6,
        2,
        "OidbSvcTrpcTcp.0x6d6_2",
        None,
        1,
        &DownloadOuter {
            download: Some(DownloadRequest {
                group_uin,
                app_id: 7,
                bus_id: 102,
                file_id: file_id.to_owned(),
            }),
        },
    )
}

/// Decodes the frozen download locator and appends the requested file identifier.
///
/// # Errors
///
/// Returns an error for rejected/malformed responses or unsafe locator fields.
pub fn parse_group_file_download_response(
    input: &[u8],
    file_id: &str,
) -> Result<String, ControlError> {
    validate(1, file_id)?;
    if !valid_file_id(file_id) {
        return Err(ControlError);
    }
    let outer = qq_wire::decode_oidb_response(input).map_err(|_error| ControlError)?;
    if outer.error_code() != 0 {
        return Err(ControlError);
    }
    let value = DownloadResponseOuter::decode(outer.body())?
        .download
        .ok_or(ControlError)?;
    if value.code != 0
        || value.dns.is_empty()
        || value.dns.len() > 253
        || !value
            .dns
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-'))
        || value.path.is_empty()
        || value.path.len() > MAX_TEXT_BYTES
    {
        return Err(ControlError);
    }
    let mut path = String::with_capacity(value.path.len() * 2);
    for byte in value.path {
        write!(path, "{byte:02x}").map_err(|_error| ControlError)?;
    }
    Ok(format!(
        "https://{}/ftn_handler/{path}/?fname={file_id}",
        value.dns
    ))
}

fn decode_entry(item: ResponseItem) -> Result<GroupFileEntry, ControlError> {
    match (item.kind, item.folder, item.file) {
        (1, None, Some(file)) if valid_file(&file) => Ok(GroupFileEntry::File(GroupFileInfo {
            file_id: file.file_id,
            file_name: file.file_name,
            file_size: file.file_size,
            bus_id: file.bus_id,
            uploaded_time: file.uploaded_time,
            expire_time: file.expire_time,
            modified_time: file.modified_time,
            downloaded_times: file.downloaded_times,
            uploader_uin: file.uploader_uin,
            uploader_name: file.uploader_name,
        })),
        (2, Some(folder), None) if valid_folder(&folder) => {
            Ok(GroupFileEntry::Folder(GroupFolderInfo {
                folder_id: folder.folder_id,
                folder_name: folder.folder_name,
                create_time: folder.create_time,
                modified_time: folder.modified_time,
                creator_uin: folder.creator_uin,
                creator_name: folder.creator_name,
                total_file_count: folder.total_file_count,
            }))
        }
        _ => Err(ControlError),
    }
}

fn valid_file(value: &FileResponse) -> bool {
    valid_text(&value.file_id)
        && valid_text(&value.file_name)
        && valid_optional_text(&value.uploader_name)
}

fn valid_folder(value: &FolderResponse) -> bool {
    valid_text(&value.folder_id)
        && valid_text(&value.folder_name)
        && valid_optional_text(&value.creator_name)
}

fn validate(group_uin: u32, value: &str) -> Result<(), ControlError> {
    if group_uin == 0 || !valid_text(value) {
        Err(ControlError)
    } else {
        Ok(())
    }
}

fn valid_text(value: &str) -> bool {
    !value.is_empty() && valid_optional_text(value)
}

fn valid_optional_text(value: &str) -> bool {
    value.len() <= MAX_TEXT_BYTES && !value.chars().any(char::is_control)
}

fn valid_file_id(value: &str) -> bool {
    value.bytes().all(|byte| {
        byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'_' | b'-' | b'.' | b'=')
    })
}

#[derive(Clone, PartialEq, Message)]
struct ViewRequest {
    #[prost(message, optional, tag = "2")]
    list: Option<ListRequest>,
}

#[derive(Clone, PartialEq, Message)]
struct ListRequest {
    #[prost(uint32, tag = "1")]
    group_uin: u32,
    #[prost(uint32, tag = "2")]
    app_id: u32,
    #[prost(string, tag = "3")]
    directory: String,
    #[prost(uint32, tag = "5")]
    count: u32,
    #[prost(uint32, tag = "9")]
    sort_by: u32,
    #[prost(uint32, tag = "13")]
    start_index: u32,
    #[prost(uint32, tag = "17")]
    field_seventeen: u32,
    #[prost(uint32, tag = "18")]
    field_eighteen: u32,
}

#[derive(Clone, PartialEq, Message)]
struct ViewResponse {
    #[prost(message, optional, tag = "2")]
    list: Option<ListResponse>,
}

#[derive(Clone, PartialEq, Message)]
struct ListResponse {
    #[prost(int32, tag = "1")]
    code: i32,
    #[prost(bool, tag = "4")]
    is_end: bool,
    #[prost(message, repeated, tag = "5")]
    items: Vec<ResponseItem>,
}

#[derive(Clone, PartialEq, Message)]
struct ResponseItem {
    #[prost(uint32, tag = "1")]
    kind: u32,
    #[prost(message, optional, tag = "2")]
    folder: Option<FolderResponse>,
    #[prost(message, optional, tag = "3")]
    file: Option<FileResponse>,
}

#[derive(Clone, PartialEq, Message)]
struct FolderResponse {
    #[prost(string, tag = "1")]
    folder_id: String,
    #[prost(string, tag = "3")]
    folder_name: String,
    #[prost(uint32, tag = "4")]
    create_time: u32,
    #[prost(uint32, tag = "5")]
    modified_time: u32,
    #[prost(uint32, tag = "6")]
    creator_uin: u32,
    #[prost(string, tag = "7")]
    creator_name: String,
    #[prost(uint32, tag = "8")]
    total_file_count: u32,
}

#[derive(Clone, PartialEq, Message)]
struct FileResponse {
    #[prost(string, tag = "1")]
    file_id: String,
    #[prost(string, tag = "2")]
    file_name: String,
    #[prost(uint64, tag = "3")]
    file_size: u64,
    #[prost(uint32, tag = "4")]
    bus_id: u32,
    #[prost(uint32, tag = "6")]
    uploaded_time: u32,
    #[prost(uint32, tag = "7")]
    expire_time: u32,
    #[prost(uint32, tag = "8")]
    modified_time: u32,
    #[prost(uint32, tag = "9")]
    downloaded_times: u32,
    #[prost(string, tag = "14")]
    uploader_name: String,
    #[prost(uint32, tag = "15")]
    uploader_uin: u32,
}

#[derive(Clone, PartialEq, Message)]
struct DownloadOuter {
    #[prost(message, optional, tag = "3")]
    download: Option<DownloadRequest>,
}

#[derive(Clone, PartialEq, Message)]
struct DownloadRequest {
    #[prost(uint32, tag = "1")]
    group_uin: u32,
    #[prost(uint32, tag = "2")]
    app_id: u32,
    #[prost(uint32, tag = "3")]
    bus_id: u32,
    #[prost(string, tag = "4")]
    file_id: String,
}

#[derive(Clone, PartialEq, Message)]
struct DownloadResponseOuter {
    #[prost(message, optional, tag = "3")]
    download: Option<DownloadResponse>,
}

#[derive(Clone, PartialEq, Message)]
struct DownloadResponse {
    #[prost(int32, tag = "1")]
    code: i32,
    #[prost(string, tag = "5")]
    dns: String,
    #[prost(bytes = "vec", tag = "6")]
    path: Vec<u8>,
}

#[cfg(test)]
mod tests {
    use prost::Message;

    use super::*;

    #[test]
    fn list_request_matches_frozen_page_shape() -> Result<(), Box<dyn std::error::Error>> {
        let request = group_file_list_request(123, "/folder", 40)?;
        let outer = qq_wire::decode_oidb_request(request.body())?;
        let list = ViewRequest::decode(outer.body())?
            .list
            .ok_or("list missing")?;
        assert_eq!(
            (outer.command(), outer.subcommand(), outer.reserved()),
            (0x6d8, 1, 1)
        );
        assert_eq!((list.group_uin, list.app_id, list.count), (123, 7, 20));
        assert_eq!((list.sort_by, list.start_index), (1, 40));
        assert_eq!((list.field_seventeen, list.field_eighteen), (2, 0));
        Ok(())
    }

    #[test]
    fn list_response_keeps_only_matching_bounded_entries() -> Result<(), Box<dyn std::error::Error>>
    {
        let body = ViewResponse {
            list: Some(ListResponse {
                code: 0,
                is_end: true,
                items: vec![ResponseItem {
                    kind: 1,
                    folder: None,
                    file: Some(FileResponse {
                        file_id: "id".to_owned(),
                        file_name: "name".to_owned(),
                        file_size: 10,
                        bus_id: 102,
                        uploaded_time: 20,
                        expire_time: 30,
                        modified_time: 40,
                        downloaded_times: 50,
                        uploader_name: "member".to_owned(),
                        uploader_uin: 60,
                    }),
                }],
            }),
        };
        let page = parse_group_file_list_response(&oidb_response(0, body.encode_to_vec()))?;
        assert!(page.is_end);
        assert!(matches!(page.entries.as_slice(), [GroupFileEntry::File(_)]));

        let ambiguous = ViewResponse {
            list: Some(ListResponse {
                code: 0,
                is_end: true,
                items: vec![ResponseItem {
                    kind: 1,
                    folder: Some(FolderResponse::default()),
                    file: Some(FileResponse::default()),
                }],
            }),
        };
        assert!(
            parse_group_file_list_response(&oidb_response(0, ambiguous.encode_to_vec())).is_err()
        );
        Ok(())
    }

    #[test]
    fn download_url_is_https_and_bound_to_requested_file() -> Result<(), Box<dyn std::error::Error>>
    {
        let body = DownloadResponseOuter {
            download: Some(DownloadResponse {
                code: 0,
                dns: "example.qq.com".to_owned(),
                path: vec![0xab, 0xcd],
            }),
        };
        assert_eq!(
            parse_group_file_download_response(
                &oidb_response(0, body.encode_to_vec()),
                "/safe_id"
            )?,
            "https://example.qq.com/ftn_handler/abcd/?fname=/safe_id"
        );
        assert!(group_file_download_request(1, "bad&id").is_err());
        Ok(())
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
