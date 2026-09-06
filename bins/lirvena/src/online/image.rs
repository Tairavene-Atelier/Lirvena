use account_api::{AccountActionError, AccountActionRequest};
use serde_json::Value;

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
