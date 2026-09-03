use account_api::{AccountActionError, AccountActionRequest};
use qq_media::AvatarTarget;
use serde_json::{Value, json};

use super::media::MediaRuntime;
use super::packets::PacketRuntime;
use super::parameters::{required_text, required_u32};
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
    let target = match request.action() {
        "set_qq_avatar" => AvatarTarget::Account,
        "set_group_portrait" => {
            AvatarTarget::Group(required_u32(request.params().get("group_id"))?)
        }
        _ => return Err(AccountActionError::ActionNotFound),
    };
    media
        .upload_avatar(file, target, packets, pushes, context)
        .await?;
    Ok(json!({}))
}
