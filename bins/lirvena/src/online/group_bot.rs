use account_api::{AccountActionError, AccountActionRequest};
use qq_control::{group_bot_callback, group_bot_status};
use serde_json::{Value, json};

use super::controls::send_control;
use super::packets::PacketRuntime;
use super::parameters::required_u32;
use super::push::PushRuntime;
use super::runtime::OnlineContext;

pub(super) async fn execute(
    request: &AccountActionRequest,
    packets: &PacketRuntime,
    pushes: &PushRuntime,
    context: &mut OnlineContext<'_>,
) -> Result<Value, AccountActionError> {
    let group_id = required_u32(request.params().get("group_id"))?;
    let bot_id = required_u32(request.params().get("bot_id"))?;
    let control = match request.action() {
        "set_group_bot_status" => {
            let enabled = required_u32(request.params().get("enable"))?;
            group_bot_status(group_id, bot_id, enabled)
        }
        "send_group_bot_callback" => group_bot_callback(
            group_id,
            bot_id,
            optional_text(request.params().get("data_1"))?,
            optional_text(request.params().get("data_2"))?,
        ),
        _ => return Err(AccountActionError::Unsupported),
    }
    .map_err(|_error| AccountActionError::BadParameters)?;
    send_control(&control, packets, pushes, context).await?;
    Ok(json!(bot_id))
}

fn optional_text(value: Option<&Value>) -> Result<&str, AccountActionError> {
    match value {
        None | Some(Value::Null) => Ok(""),
        Some(Value::String(value)) => Ok(value),
        Some(_) => Err(AccountActionError::BadParameters),
    }
}
