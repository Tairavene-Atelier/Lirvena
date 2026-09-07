use account_api::{AccountActionError, AccountActionRequest};
use qq_control::{image_ocr, parse_image_ocr_response};
use qq_media::{MediaRkeyKind, encode_media_rkey_request, parse_media_rkey_response};
use serde_json::{Value, json};

use super::controls::send_control_response;
use super::media::MediaRuntime;
use super::packets::PacketRuntime;
use super::parameters::required_text;
use super::push::PushRuntime;
use super::runtime::OnlineContext;

pub(super) async fn upload(
    request: &AccountActionRequest,
    media: &mut MediaRuntime,
    packets: &PacketRuntime,
    pushes: &PushRuntime,
    context: &mut OnlineContext<'_>,
) -> Result<Value, AccountActionError> {
    let file = required_text(request.params().get("file"))?;
    media
        .upload_image_url(file, packets, pushes, context)
        .await
        .map(Value::String)
}

pub(super) async fn ocr(
    request: &AccountActionRequest,
    media: &mut MediaRuntime,
    packets: &PacketRuntime,
    pushes: &PushRuntime,
    context: &mut OnlineContext<'_>,
) -> Result<Value, AccountActionError> {
    let image = required_text(request.params().get("image"))?;
    let image_url = media
        .upload_image_url(image, packets, pushes, context)
        .await?;
    let request = image_ocr(&image_url).map_err(|_error| AccountActionError::QqFailure)?;
    let response = send_control_response(&request, packets, pushes, context).await?;
    let result =
        parse_image_ocr_response(&response).map_err(|_error| AccountActionError::QqFailure)?;
    let texts = result
        .texts
        .into_iter()
        .map(|detection| {
            json!({
                "text": detection.text,
                "confidence": detection.confidence,
                "coordinates": detection.coordinates.into_iter().map(|coordinate| json!({
                    "x": coordinate.x,
                    "y": coordinate.y,
                })).collect::<Vec<_>>(),
            })
        })
        .collect::<Vec<_>>();
    Ok(json!({"texts": texts, "language": result.language}))
}

pub(super) async fn rkeys(
    packets: &PacketRuntime,
    pushes: &PushRuntime,
    context: &mut OnlineContext<'_>,
) -> Result<Value, AccountActionError> {
    let request = encode_media_rkey_request().map_err(|_error| AccountActionError::QqFailure)?;
    let response = packets
        .send_with_reserve(
            super::packets::PacketContext::for_account(context, pushes.plan()),
            request.command(),
            &[],
            request.body(),
        )
        .await
        .map_err(|_error| AccountActionError::QqFailure)?;
    let values =
        parse_media_rkey_response(&response).map_err(|_error| AccountActionError::QqFailure)?;
    Ok(json!({
        "rkeys": values.into_iter().map(|value| json!({
            "type": match value.kind {
                MediaRkeyKind::Private => "private",
                MediaRkeyKind::Group => "group",
            },
            "rkey": value.value,
            "created_at": value.created_at,
            "ttl": value.ttl_seconds,
        })).collect::<Vec<_>>(),
    }))
}
