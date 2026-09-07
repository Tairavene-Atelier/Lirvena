use account_api::{AccountActionError, AccountActionRequest};
use qq_control::{
    GroupFileControl, GroupFileEntry, GroupFileUploadSpec, create_group_file_folder,
    delete_group_file, delete_group_file_folder, group_file_complete_request,
    group_file_download_request, group_file_list_request, group_file_upload_request,
    move_group_file, parse_group_file_download_response, parse_group_file_list_response,
    parse_group_file_upload_response, rename_group_file_folder,
};
use qq_media::MediaReference;
use serde_json::{Value, json};

use super::controls::{send_control, send_control_response};
use super::media::MediaRuntime;
use super::packets::PacketRuntime;
use super::parameters::{file_name, required_text, required_u32};
use super::push::PushRuntime;
use super::runtime::OnlineContext;
use crate::support::random_nonzero_u32;

const MAX_LIST_PAGES: u32 = 256;
const GROUP_FILE_HIGHWAY_COMMAND: u32 = 71;

pub(super) async fn resolve_notice(
    identity: &account_api::AccountIdentity,
    group_id: u32,
    sender_id: u32,
    file: &qq_message::GroupFileSegment,
    occurred_at: u64,
    packets: &PacketRuntime,
    pushes: &PushRuntime,
    context: &mut OnlineContext<'_>,
) -> Result<account_api::ResolvedGroupFile, AccountActionError> {
    let control = group_file_download_request(group_id, file.file_id())
        .map_err(|_error| AccountActionError::QqFailure)?;
    let response = send_control_response(&control, packets, pushes, context).await?;
    let url = parse_group_file_download_response(&response, file.file_id())
        .map_err(|_error| AccountActionError::QqFailure)?;
    account_api::ResolvedGroupFile::new(
        identity.clone(),
        u64::from(group_id),
        u64::from(sender_id),
        file.bus_id(),
        file.file_id().to_owned(),
        file.name().to_owned(),
        file.size(),
        url,
        occurred_at,
    )
    .map_err(|_error| AccountActionError::QqFailure)
}

pub(super) async fn upload(
    request: &AccountActionRequest,
    media: &mut MediaRuntime,
    packets: &PacketRuntime,
    pushes: &PushRuntime,
    context: &mut OnlineContext<'_>,
) -> Result<Value, AccountActionError> {
    let params = request.params();
    let group_uin = required_u32(params.get("group_id"))?;
    let reference = MediaReference::parse(required_text(params.get("file"))?)
        .map_err(|_error| AccountActionError::BadParameters)?;
    let file_name = file_name(params.get("name"), reference.suggested_file_name())?;
    let target_directory = params
        .get("folder")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .unwrap_or("/");
    let object = media.resolve(&reference).await?;
    let file_size =
        u64::try_from(object.bytes().len()).map_err(|_error| AccountActionError::BadParameters)?;
    let upload = group_file_upload_request(&GroupFileUploadSpec {
        group_uin,
        target_directory,
        file_name,
        file_size,
        sha1: &object.sha1(),
        md5: &object.md5(),
    })
    .map_err(|_error| AccountActionError::BadParameters)?;
    let response = send_control_response(&upload, packets, pushes, context).await?;
    let plan = parse_group_file_upload_response(&response)
        .map_err(|_error| AccountActionError::QqFailure)?;
    if !plan.file_exists() {
        let extension = plan
            .highway_extension(context.uin, group_uin, file_name, file_size, &object.md5())
            .map_err(|_error| AccountActionError::QqFailure)?;
        media
            .upload_bytes(
                GROUP_FILE_HIGHWAY_COMMAND,
                &extension,
                object.bytes(),
                packets,
                pushes,
                context,
            )
            .await?;
    }
    let completion = group_file_complete_request(
        group_uin,
        plan.file_id(),
        random_nonzero_u32().map_err(|_error| AccountActionError::QqFailure)?,
    )
    .map_err(|_error| AccountActionError::QqFailure)?;
    send_control(&completion, packets, pushes, context).await?;
    Ok(json!({}))
}

pub(super) async fn list(
    request: &AccountActionRequest,
    packets: &PacketRuntime,
    pushes: &PushRuntime,
    context: &mut OnlineContext<'_>,
) -> Result<Value, AccountActionError> {
    let group_uin = required_u32(request.params().get("group_id"))?;
    let directory = match request.action() {
        "get_group_root_files" => "/",
        "get_group_files_by_folder" => required_text(request.params().get("folder_id"))?,
        _ => return Err(AccountActionError::ActionNotFound),
    };
    let mut files = Vec::new();
    let mut folders = Vec::new();
    for page_index in 0..MAX_LIST_PAGES {
        let start_index = page_index
            .checked_mul(20)
            .ok_or(AccountActionError::QqFailure)?;
        let query = group_file_list_request(group_uin, directory, start_index)
            .map_err(|_error| AccountActionError::BadParameters)?;
        let response = send_control_response(&query, packets, pushes, context).await?;
        let page = parse_group_file_list_response(&response)
            .map_err(|_error| AccountActionError::QqFailure)?;
        let is_empty = page.entries.is_empty();
        for entry in page.entries {
            match entry {
                GroupFileEntry::File(file) => files.push(json!({
                    "group_id": group_uin,
                    "file_id": file.file_id,
                    "file_name": file.file_name,
                    "busid": 0,
                    "file_size": file.file_size,
                    "upload_time": file.uploaded_time,
                    "dead_time": file.expire_time,
                    "modify_time": file.modified_time,
                    "download_times": file.downloaded_times,
                    "uploader": file.uploader_uin,
                    "uploader_name": file.uploader_name,
                })),
                GroupFileEntry::Folder(folder) => folders.push(json!({
                    "group_id": group_uin,
                    "folder_id": folder.folder_id,
                    "folder_name": folder.folder_name,
                    "create_time": folder.create_time,
                    "creator": folder.creator_uin,
                    "create_name": folder.creator_name,
                    "total_file_count": folder.total_file_count,
                })),
            }
        }
        if page.is_end {
            return Ok(json!({"files": files, "folders": folders}));
        }
        if is_empty {
            return Err(AccountActionError::QqFailure);
        }
    }
    Err(AccountActionError::QqFailure)
}

pub(super) async fn download_url(
    request: &AccountActionRequest,
    packets: &PacketRuntime,
    pushes: &PushRuntime,
    context: &mut OnlineContext<'_>,
) -> Result<Value, AccountActionError> {
    let group_uin = required_u32(request.params().get("group_id"))?;
    let file_id = required_text(request.params().get("file_id"))?;
    let query = group_file_download_request(group_uin, file_id)
        .map_err(|_error| AccountActionError::BadParameters)?;
    let response = send_control_response(&query, packets, pushes, context).await?;
    let url = parse_group_file_download_response(&response, file_id)
        .map_err(|_error| AccountActionError::QqFailure)?;
    Ok(json!({"url": url}))
}

pub(super) async fn mutate(
    request: &AccountActionRequest,
    packets: &PacketRuntime,
    pushes: &PushRuntime,
    context: &mut OnlineContext<'_>,
) -> Result<Value, AccountActionError> {
    let params = request.params();
    let group_uin = required_u32(params.get("group_id"))?;
    let control = match request.action() {
        "delete_group_file" => delete_group_file(group_uin, required_text(params.get("file_id"))?),
        "move_group_file" => move_group_file(
            group_uin,
            required_text(params.get("file_id"))?,
            required_text(params.get("parent_directory"))?,
            required_text(params.get("target_directory"))?,
        ),
        "create_group_file_folder" => {
            create_group_file_folder(group_uin, required_text(params.get("name"))?)
        }
        "delete_group_file_folder" => {
            delete_group_file_folder(group_uin, required_text(params.get("folder_id"))?)
        }
        "rename_group_file_folder" => rename_group_file_folder(
            group_uin,
            required_text(params.get("folder_id"))?,
            required_text(params.get("new_folder_name"))?,
        ),
        _ => return Err(AccountActionError::ActionNotFound),
    }
    .map_err(|_error| AccountActionError::BadParameters)?;
    send(&control, packets, pushes, context).await?;
    Ok(json!({}))
}

async fn send(
    control: &GroupFileControl,
    packets: &PacketRuntime,
    pushes: &PushRuntime,
    context: &mut OnlineContext<'_>,
) -> Result<(), AccountActionError> {
    let response = send_control_response(control.request(), packets, pushes, context).await?;
    control
        .parse_response(&response)
        .map_err(|_error| AccountActionError::QqFailure)
}
