use account_api::{AccountActionError, AccountActionRequest};
use qq_control::{
    fetch_custom_faces, fetch_market_face_keys, parse_custom_faces_response,
    parse_market_face_keys_response,
};
use serde_json::{Value, json};

use super::controls::send_control_response;
use super::packets::PacketRuntime;
use super::push::PushRuntime;
use super::runtime::OnlineContext;

pub(super) async fn execute(
    request: &AccountActionRequest,
    packets: &PacketRuntime,
    pushes: &PushRuntime,
    context: &mut OnlineContext<'_>,
) -> Result<Value, AccountActionError> {
    match request.action() {
        "fetch_custom_face" => custom_faces(packets, pushes, context).await,
        "fetch_mface_key" => market_face_keys(request, packets, pushes, context).await,
        _ => Err(AccountActionError::ActionNotFound),
    }
}

async fn custom_faces(
    packets: &PacketRuntime,
    pushes: &PushRuntime,
    context: &mut OnlineContext<'_>,
) -> Result<Value, AccountActionError> {
    let account_uin = u32::try_from(context.uin).map_err(|_error| AccountActionError::QqFailure)?;
    let request = fetch_custom_faces(
        account_uin,
        context.device.profile().kernel_version(),
        context.profile.client_version(),
    )
    .map_err(|_error| AccountActionError::QqFailure)?;
    let response = send_control_response(&request, packets, pushes, context).await?;
    parse_custom_faces_response(&response, account_uin)
        .map(|values| json!(values))
        .map_err(|_error| AccountActionError::QqFailure)
}

async fn market_face_keys(
    action: &AccountActionRequest,
    packets: &PacketRuntime,
    pushes: &PushRuntime,
    context: &mut OnlineContext<'_>,
) -> Result<Value, AccountActionError> {
    let values = action
        .params()
        .get("emoji_ids")
        .and_then(Value::as_array)
        .ok_or(AccountActionError::BadParameters)?;
    let ids = values
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::to_owned)
                .ok_or(AccountActionError::BadParameters)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let request =
        fetch_market_face_keys(&ids).map_err(|_error| AccountActionError::BadParameters)?;
    let response = send_control_response(&request, packets, pushes, context).await?;
    parse_market_face_keys_response(&response)
        .map(|values| json!(values))
        .map_err(|_error| AccountActionError::QqFailure)
}
