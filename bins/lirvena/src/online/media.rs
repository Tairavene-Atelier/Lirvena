use std::path::Path;
use std::time::{Duration, Instant};

use account_api::AccountActionError;
use qq_highway::{HighwayClient, HighwaySession, UploadIdentity};
use qq_media::{
    AvatarTarget, MediaPolicy, MediaReference, MediaResolver, MediaTarget, RemoteMediaPolicy,
    RichMediaUploadPlan, analyze_image, analyze_video, avatar_upload, default_video_thumbnail,
    encode_image_metadata_request, encode_record_metadata_request, encode_video_metadata_request,
    parse_image_metadata_response, parse_record_metadata_response, parse_video_metadata_response,
    prepare_record,
};

use super::packets::{PacketContext, PacketRuntime};
use super::push::PushRuntime;
use super::runtime::OnlineContext;
use crate::support::random_nonzero_u32;

const MAX_MEDIA_BYTES: usize = 256 * 1024 * 1024;
const SESSION_LIFETIME: Duration = Duration::from_hours(12);

pub(super) struct UploadedImage {
    pub group: bool,
    pub message_info: Vec<u8>,
    pub compatibility: Vec<u8>,
}

pub(super) struct UploadedRecord {
    pub group: bool,
    pub message_info: Vec<u8>,
}

pub(super) struct UploadedVideo {
    pub group: bool,
    pub message_info: Vec<u8>,
    pub compatibility: Vec<u8>,
}

pub(super) struct MediaRuntime {
    resolver: MediaResolver,
    highway: HighwayClient,
    session: Option<(Instant, HighwaySession)>,
}

impl MediaRuntime {
    pub(super) fn new(state_directory: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let remote = RemoteMediaPolicy::public_web(Duration::from_secs(15))?;
        let policy = MediaPolicy::new(
            vec![state_directory.to_owned()],
            Some(state_directory.join("media-cache")),
            MAX_MEDIA_BYTES,
            Some(remote),
        )?;
        Ok(Self {
            resolver: MediaResolver::new(policy),
            highway: HighwayClient::new()?,
            session: None,
        })
    }

    pub(super) async fn upload_image(
        &mut self,
        reference: &str,
        target: MediaTarget<'_>,
        packets: &PacketRuntime,
        pushes: &PushRuntime,
        context: &mut OnlineContext<'_>,
    ) -> Result<UploadedImage, AccountActionError> {
        let reference =
            MediaReference::parse(reference).map_err(|_error| AccountActionError::BadParameters)?;
        let object = self
            .resolver
            .resolve(&reference)
            .await
            .map_err(|_error| AccountActionError::QqFailure)?;
        let descriptor =
            analyze_image(object.bytes()).map_err(|_error| AccountActionError::BadParameters)?;
        let request = encode_image_metadata_request(
            target,
            &descriptor,
            random_nonzero_u32().map_err(|_error| AccountActionError::QqFailure)?,
        )
        .map_err(|_error| AccountActionError::BadParameters)?;
        let response = packets
            .send_with_reserve(
                PacketContext::for_account(context, pushes.plan()),
                request.command(),
                &[],
                request.body(),
            )
            .await
            .map_err(|_error| AccountActionError::QqFailure)?;
        let plan = parse_image_metadata_response(&response, &target)
            .map_err(|_error| AccountActionError::QqFailure)?;
        self.complete_upload(&plan, &[object.bytes()], packets, pushes, context)
            .await?;
        let (message_info, compatibility) = plan.into_message_material();
        Ok(UploadedImage {
            group: matches!(target, MediaTarget::Group(_)),
            message_info,
            compatibility,
        })
    }

    pub(super) async fn upload_avatar(
        &mut self,
        reference: &str,
        target: AvatarTarget,
        packets: &PacketRuntime,
        pushes: &PushRuntime,
        context: &mut OnlineContext<'_>,
    ) -> Result<(), AccountActionError> {
        let reference =
            MediaReference::parse(reference).map_err(|_error| AccountActionError::BadParameters)?;
        let object = self
            .resolver
            .resolve(&reference)
            .await
            .map_err(|_error| AccountActionError::QqFailure)?;
        analyze_image(object.bytes()).map_err(|_error| AccountActionError::BadParameters)?;
        let upload = avatar_upload(target).map_err(|_error| AccountActionError::BadParameters)?;
        self.upload_bytes(
            upload.command_id(),
            upload.extension(),
            object.bytes(),
            packets,
            pushes,
            context,
        )
        .await
    }

    pub(super) async fn upload_record(
        &mut self,
        reference: &str,
        target: MediaTarget<'_>,
        packets: &PacketRuntime,
        pushes: &PushRuntime,
        context: &mut OnlineContext<'_>,
    ) -> Result<UploadedRecord, AccountActionError> {
        let reference =
            MediaReference::parse(reference).map_err(|_error| AccountActionError::BadParameters)?;
        let object = self
            .resolver
            .resolve(&reference)
            .await
            .map_err(|_error| AccountActionError::QqFailure)?;
        let record =
            prepare_record(object.bytes()).map_err(|_error| AccountActionError::BadParameters)?;
        let request = encode_record_metadata_request(
            target,
            record.descriptor(),
            random_nonzero_u32().map_err(|_error| AccountActionError::QqFailure)?,
        )
        .map_err(|_error| AccountActionError::BadParameters)?;
        let response = packets
            .send_with_reserve(
                PacketContext::for_account(context, pushes.plan()),
                request.command(),
                &[],
                request.body(),
            )
            .await
            .map_err(|_error| AccountActionError::QqFailure)?;
        let plan = parse_record_metadata_response(&response, &target)
            .map_err(|_error| AccountActionError::QqFailure)?;
        self.complete_upload(&plan, &[record.bytes()], packets, pushes, context)
            .await?;
        let (message_info, _compatibility) = plan.into_message_material();
        Ok(UploadedRecord {
            group: matches!(target, MediaTarget::Group(_)),
            message_info,
        })
    }

    pub(super) async fn upload_video(
        &mut self,
        reference: &str,
        target: MediaTarget<'_>,
        packets: &PacketRuntime,
        pushes: &PushRuntime,
        context: &mut OnlineContext<'_>,
    ) -> Result<UploadedVideo, AccountActionError> {
        let reference =
            MediaReference::parse(reference).map_err(|_error| AccountActionError::BadParameters)?;
        let object = self
            .resolver
            .resolve(&reference)
            .await
            .map_err(|_error| AccountActionError::QqFailure)?;
        let video =
            analyze_video(object.bytes()).map_err(|_error| AccountActionError::BadParameters)?;
        let thumbnail_bytes = default_video_thumbnail();
        let thumbnail =
            analyze_image(thumbnail_bytes).map_err(|_error| AccountActionError::QqFailure)?;
        let request = encode_video_metadata_request(
            target,
            &video,
            &thumbnail,
            random_nonzero_u32().map_err(|_error| AccountActionError::QqFailure)?,
        )
        .map_err(|_error| AccountActionError::BadParameters)?;
        let response = packets
            .send_with_reserve(
                PacketContext::for_account(context, pushes.plan()),
                request.command(),
                &[],
                request.body(),
            )
            .await
            .map_err(|_error| AccountActionError::QqFailure)?;
        let plan = parse_video_metadata_response(&response, &target)
            .map_err(|_error| AccountActionError::QqFailure)?;
        self.complete_upload(
            &plan,
            &[object.bytes(), thumbnail_bytes],
            packets,
            pushes,
            context,
        )
        .await?;
        let (message_info, compatibility) = plan.into_message_material();
        Ok(UploadedVideo {
            group: matches!(target, MediaTarget::Group(_)),
            message_info,
            compatibility,
        })
    }

    async fn complete_upload(
        &mut self,
        plan: &RichMediaUploadPlan,
        files: &[&[u8]],
        packets: &PacketRuntime,
        pushes: &PushRuntime,
        context: &mut OnlineContext<'_>,
    ) -> Result<(), AccountActionError> {
        for upload in plan.uploads() {
            let bytes = files
                .get(upload.file_index())
                .ok_or(AccountActionError::QqFailure)?;
            let extension = upload
                .extension_for(bytes)
                .map_err(|_error| AccountActionError::QqFailure)?;
            self.upload_bytes(
                upload.command_id(),
                &extension,
                bytes,
                packets,
                pushes,
                context,
            )
            .await?;
        }
        Ok(())
    }

    async fn upload_bytes(
        &mut self,
        command_id: u32,
        extension: &[u8],
        bytes: &[u8],
        packets: &PacketRuntime,
        pushes: &PushRuntime,
        context: &mut OnlineContext<'_>,
    ) -> Result<(), AccountActionError> {
        self.ensure_session(packets, pushes, context).await?;
        let session = self
            .session
            .as_ref()
            .map(|(_, session)| session)
            .ok_or(AccountActionError::QqFailure)?;
        self.highway
            .upload(
                session,
                &UploadIdentity {
                    uin: context.uin,
                    app_id: context.profile.app_id(),
                    sub_app_id: context.profile.sub_app_id(),
                    login_signature: context.credential.secrets().tgt(),
                },
                command_id,
                extension,
                bytes,
            )
            .await
            .map_err(|_error| {
                self.session = None;
                AccountActionError::QqFailure
            })?;
        Ok(())
    }

    async fn ensure_session(
        &mut self,
        packets: &PacketRuntime,
        pushes: &PushRuntime,
        context: &mut OnlineContext<'_>,
    ) -> Result<(), AccountActionError> {
        if self
            .session
            .as_ref()
            .is_some_and(|(created_at, _)| created_at.elapsed() < SESSION_LIFETIME)
        {
            return Ok(());
        }
        let request = qq_highway::encode_session_request(context.credential.secrets().tgt())
            .map_err(|_error| AccountActionError::QqFailure)?;
        let response = packets
            .send_with_reserve(
                PacketContext::for_account(context, pushes.plan()),
                "HttpConn.0x6ff_501",
                &[],
                &request,
            )
            .await
            .map_err(|_error| AccountActionError::QqFailure)?;
        let session = qq_highway::decode_session_response(&response)
            .map_err(|_error| AccountActionError::QqFailure)?;
        self.session = Some((Instant::now(), session));
        Ok(())
    }
}
