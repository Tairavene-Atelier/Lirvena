use account_api::{AccountActionError, AccountActionRequest};
use qq_control::{image_ocr, parse_image_ocr_response};
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
