use account_api::{AccountActionError, AccountActionRequest};
use qq_control::{
    GroupFileControl, create_group_file_folder, delete_group_file, delete_group_file_folder,
    move_group_file, rename_group_file_folder,
};
use serde_json::{Value, json};

use super::controls::send_control_response;
use super::packets::PacketRuntime;
use super::parameters::{required_text, required_u32};
use super::push::PushRuntime;
use super::runtime::OnlineContext;

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
