use std::collections::BTreeMap;

use account_api::{AccountActionError, AccountActionRequest};
use qq_control::{parse_private_file_url_response, private_file_url};
use qq_directory::FriendEntry;
use serde_json::{Value, json};

use super::actions::resolve_private_uid;
use super::controls::send_control_response;
use super::packets::PacketRuntime;
use super::parameters::{required_text, required_u32};
use super::push::PushRuntime;
use super::runtime::OnlineContext;

pub(super) async fn download_url(
    request: &AccountActionRequest,
    packets: &PacketRuntime,
    pushes: &PushRuntime,
    friends: &mut BTreeMap<u32, FriendEntry>,
    context: &mut OnlineContext<'_>,
) -> Result<Value, AccountActionError> {
    let user_id = required_u32(request.params().get("user_id"))?;
    let file_id = required_text(request.params().get("file_id"))?;
    let file_hash = required_text(request.params().get("file_hash"))?;
    let uid = resolve_private_uid(user_id, packets, pushes, friends, context).await?;
    let control = private_file_url(&uid, file_id, file_hash)
        .map_err(|_error| AccountActionError::BadParameters)?;
    let response = send_control_response(&control, packets, pushes, context).await?;
    let url = parse_private_file_url_response(&response)
        .map_err(|_error| AccountActionError::QqFailure)?;
    Ok(json!({"url": url}))
}
