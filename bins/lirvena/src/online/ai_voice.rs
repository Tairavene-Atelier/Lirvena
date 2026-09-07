use std::time::Duration;

use account_api::{AccountActionError, AccountActionRequest, AccountIdentity};
use qq_control::{
    AiVoiceGeneration, ai_voice_characters, generate_ai_voice, parse_ai_voice_characters_response,
    parse_ai_voice_generation_response,
};
use qq_media::{encode_group_record_download_request, parse_group_record_download_response};
use qq_message::SendTextTarget;
use serde_json::{Value, json};
use tokio::time::sleep;

use super::actions::{CompiledSegment, send_segments};
use super::controls::send_control_response;
use super::message_registry::MessageRegistry;
use super::packets::{PacketContext, PacketRuntime};
use super::parameters::{optional_u32, required_text, required_u32};
use super::push::PushRuntime;
use super::runtime::OnlineContext;
use crate::support::random_nonzero_u32;

const MAX_GENERATION_POLLS: usize = 30;
const GENERATION_POLL_INTERVAL: Duration = Duration::from_secs(2);

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

pub(super) async fn record_url(
    request: &AccountActionRequest,
    packets: &PacketRuntime,
    pushes: &PushRuntime,
    context: &mut OnlineContext<'_>,
) -> Result<Value, AccountActionError> {
    let (group_id, message_info, _source) = generate(request, packets, pushes, context).await?;
    let download = encode_group_record_download_request(&message_info, group_id)
        .map_err(|_error| AccountActionError::QqFailure)?;
    let response = packets
        .send_with_reserve(
            PacketContext::for_account(context, pushes.plan()),
            download.command(),
            &[],
            download.body(),
        )
        .await
        .map_err(|_error| AccountActionError::QqFailure)?;
    parse_group_record_download_response(&response)
        .map(Value::String)
        .map_err(|_error| AccountActionError::QqFailure)
}

pub(super) async fn send_record(
    request: &AccountActionRequest,
    identity: &AccountIdentity,
    packets: &PacketRuntime,
    pushes: &PushRuntime,
    messages: &mut MessageRegistry,
    context: &mut OnlineContext<'_>,
) -> Result<Value, AccountActionError> {
    let (group_id, message_info, source) = generate(request, packets, pushes, context).await?;
    let segment = CompiledSegment::Record {
        source,
        group: true,
        message_info,
    };
    send_segments(
        SendTextTarget::Group {
            group_code: group_id,
        },
        &[segment],
        identity,
        packets,
        pushes,
        messages,
        context,
    )
    .await
}

async fn generate(
    request: &AccountActionRequest,
    packets: &PacketRuntime,
    pushes: &PushRuntime,
    context: &mut OnlineContext<'_>,
) -> Result<(u32, Vec<u8>, String), AccountActionError> {
    let group_id = required_u32(request.params().get("group_id"))?;
    let character_id = required_text(request.params().get("character"))?;
    let control = generate_ai_voice(
        group_id,
        character_id,
        required_text(request.params().get("text"))?,
        optional_u32(request.params().get("chat_type"), 1)?,
        random_nonzero_u32().map_err(|_error| AccountActionError::QqFailure)?,
    )
    .map_err(|_error| AccountActionError::BadParameters)?;
    for attempt in 0..MAX_GENERATION_POLLS {
        let response = send_control_response(&control, packets, pushes, context).await?;
        match parse_ai_voice_generation_response(&response)
            .map_err(|_error| AccountActionError::QqFailure)?
        {
            AiVoiceGeneration::Ready(message_info) => {
                return Ok((group_id, message_info, format!("qq-ai:{character_id}")));
            }
            AiVoiceGeneration::Pending if attempt + 1 < MAX_GENERATION_POLLS => {
                sleep(GENERATION_POLL_INTERVAL).await;
            }
            AiVoiceGeneration::Pending => return Err(AccountActionError::QqFailure),
        }
    }
    Err(AccountActionError::QqFailure)
}
