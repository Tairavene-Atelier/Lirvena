use account_api::{AccountActionError, AccountActionRequest};
use qq_control::{ai_voice_characters, parse_ai_voice_characters_response};
use serde_json::{Value, json};

use super::controls::send_control_response;
use super::packets::PacketRuntime;
use super::parameters::{optional_u32, required_u32};
use super::push::PushRuntime;
use super::runtime::OnlineContext;

pub(super) async fn characters(
    request: &AccountActionRequest,
    packets: &PacketRuntime,
    pushes: &PushRuntime,
    context: &mut OnlineContext<'_>,
) -> Result<Value, AccountActionError> {
    let control = ai_voice_characters(
        required_u32(request.params().get("group_id"))?,
        optional_u32(request.params().get("chat_type"), 1)?,
    )
    .map_err(|_error| AccountActionError::BadParameters)?;
    let response = send_control_response(&control, packets, pushes, context).await?;
    let categories = parse_ai_voice_characters_response(&response)
        .map_err(|_error| AccountActionError::QqFailure)?;
    Ok(Value::Array(
        categories
            .into_iter()
            .map(|category| {
                json!({
                    "type": category.kind,
                    "characters": category.characters.into_iter().map(|character| json!({
                        "character_id": character.character_id,
                        "character_name": character.character_name,
                        "preview_url": character.preview_url,
                    })).collect::<Vec<_>>(),
                })
            })
            .collect(),
    ))
}
