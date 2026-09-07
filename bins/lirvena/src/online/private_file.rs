use std::collections::BTreeMap;

use account_api::{AccountActionError, AccountActionRequest};
use qq_control::{
    PrivateFileUploadSpec, parse_private_file_upload_response, parse_private_file_url_response,
    private_file_upload_request, private_file_url,
};
use qq_directory::FriendEntry;
use qq_media::MediaReference;
use qq_message::{
    PrivateFileMessageInput, encode_private_file_message, validate_private_file_message_response,
};
use serde_json::{Value, json};

use super::actions::resolve_private_uid;
use super::controls::send_control_response;
use super::media::MediaRuntime;
use super::packets::{PacketContext, PacketRuntime};
use super::parameters::{file_name, required_text, required_u32};
use super::push::PushRuntime;
use super::runtime::OnlineContext;
use crate::opaque::{OpaqueOperation, request_reserve};
use crate::support::{now_seconds, random_nonzero_u32};

const PRIVATE_FILE_HIGHWAY_COMMAND: u32 = 95;
const FILE_LIFETIME_SECONDS: u32 = 7 * 24 * 60 * 60;
const PREFIX_DIGEST_BYTES: usize = 10 * 1024 * 1024;

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

pub(super) async fn upload(
    request: &AccountActionRequest,
    packets: &PacketRuntime,
    pushes: &PushRuntime,
    friends: &mut BTreeMap<u32, FriendEntry>,
    media: &mut MediaRuntime,
    context: &mut OnlineContext<'_>,
) -> Result<Value, AccountActionError> {
    let params = request.params();
    let user_id = required_u32(params.get("user_id"))?;
    let reference = MediaReference::parse(required_text(params.get("file"))?)
        .map_err(|_error| AccountActionError::BadParameters)?;
    let file_name = file_name(params.get("name"), reference.suggested_file_name())?;
    let recipient_uid = resolve_private_uid(user_id, packets, pushes, friends, context).await?;
    let object = media.resolve(&reference).await?;
    let file_size =
        u32::try_from(object.bytes().len()).map_err(|_error| AccountActionError::BadParameters)?;
    let upload = private_file_upload_request(&PrivateFileUploadSpec {
        sender_uid: context.credential.uid(),
        receiver_uid: &recipient_uid,
        file_name,
        file_size,
        prefix_md5: &object.md5_prefix(PREFIX_DIGEST_BYTES),
        sha1: &object.sha1(),
        md5: &object.md5(),
    })
    .map_err(|_error| AccountActionError::BadParameters)?;
    let response = send_control_response(&upload, packets, pushes, context).await?;
    let plan = parse_private_file_upload_response(&response)
        .map_err(|_error| AccountActionError::QqFailure)?;
    if !plan.file_exists() {
        let extension = plan
            .highway_extension(
                context.uin,
                file_name,
                u64::from(file_size),
                &object.md5(),
                &object.sha1(),
            )
            .map_err(|_error| AccountActionError::QqFailure)?;
        media
            .upload_bytes(
                PRIVATE_FILE_HIGHWAY_COMMAND,
                &extension,
                object.bytes(),
                packets,
                pushes,
                context,
            )
            .await?;
    }
    let unix_seconds = now_seconds().map_err(|_error| AccountActionError::QqFailure)?;
    let expire_at = unix_seconds
        .checked_add(FILE_LIFETIME_SECONDS)
        .ok_or(AccountActionError::QqFailure)?;
    let body = encode_private_file_message(&PrivateFileMessageInput {
        recipient_uid: &recipient_uid,
        file_id: plan.file_id(),
        file_hash: plan.file_hash(),
        file_name,
        file_size: u64::from(file_size),
        md5: &object.md5(),
        expire_at,
        client_sequence: random_nonzero_u32().map_err(|_error| AccountActionError::QqFailure)?,
        random: random_nonzero_u32().map_err(|_error| AccountActionError::QqFailure)?,
        unix_seconds,
    })
    .map_err(|_error| AccountActionError::QqFailure)?;
    let reserve = request_reserve(
        context.ceylith,
        context.account_slot_id,
        OpaqueOperation::C,
        &body,
    )
    .await
    .map_err(|_error| AccountActionError::QqFailure)?;
    let response = packets
        .send_with_reserve(
            PacketContext::for_account(context, pushes.plan()),
            "MessageSvc.PbSendMsg",
            &reserve,
            &body,
        )
        .await
        .map_err(|_error| AccountActionError::QqFailure)?;
    validate_private_file_message_response(&response)
        .map_err(|_error| AccountActionError::QqFailure)?;
    Ok(json!({}))
}
