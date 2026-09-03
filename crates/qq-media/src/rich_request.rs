use prost::Message;

use crate::image::valid_uid;
use crate::image_proto::{
    BusinessInfo, ClientMeta, CommonHead, DirectTarget, GroupTarget, PictureBusiness, RequestHead,
    RichRequest, Scene, UploadInfo, UploadRequest, VideoBusiness, VoiceBusiness,
};
use crate::{MediaError, MediaTarget};

pub(crate) struct RichRequestSpec<'a> {
    pub target: MediaTarget<'a>,
    pub client_random_id: u32,
    pub direct_route: (&'static str, u32),
    pub group_route: (&'static str, u32),
    pub business_type: u32,
    pub files: Vec<UploadInfo>,
    pub picture: PictureBusiness,
    pub video: VideoBusiness,
    pub direct_voice: VoiceBusiness,
    pub group_voice: VoiceBusiness,
}

pub(crate) struct EncodedRichRequest {
    pub command: &'static str,
    pub body: Vec<u8>,
}

pub(crate) fn encode(spec: RichRequestSpec<'_>) -> Result<EncodedRichRequest, MediaError> {
    if spec.client_random_id == 0 {
        return Err(MediaError::ReferenceRejected);
    }
    let (command, oidb_command, scene_type, direct, group, voice) = match spec.target {
        MediaTarget::Direct(uid) if valid_uid(uid) => (
            spec.direct_route.0,
            spec.direct_route.1,
            1,
            Some(DirectTarget {
                account_type: 2,
                uid: uid.to_owned(),
            }),
            None,
            spec.direct_voice,
        ),
        MediaTarget::Group(group_code) if group_code != 0 => (
            spec.group_route.0,
            spec.group_route.1,
            2,
            None,
            Some(GroupTarget { group_code }),
            spec.group_voice,
        ),
        MediaTarget::Direct(_) | MediaTarget::Group(_) => {
            return Err(MediaError::ReferenceRejected);
        }
    };
    let inner = RichRequest {
        head: Some(RequestHead {
            common: Some(CommonHead {
                request_id: 1,
                command: 100,
            }),
            scene: Some(Scene {
                request_type: 2,
                business_type: spec.business_type,
                kind: scene_type,
                direct,
                group,
            }),
            client: Some(ClientMeta { agent_type: 2 }),
        }),
        upload: Some(UploadRequest {
            files: spec.files,
            try_fast_upload: true,
            server_sends_message: false,
            client_random_id: u64::from(spec.client_random_id),
            compatibility_scene: scene_type,
            business: Some(BusinessInfo {
                picture: Some(spec.picture),
                video: Some(spec.video),
                voice: Some(voice),
            }),
            client_sequence: 10,
            no_compatibility_message: false,
        }),
    }
    .encode_to_vec();
    let body = qq_wire::encode_oidb_request(oidb_command, 100, &inner, 1)
        .map_err(|_error| MediaError::ReferenceRejected)?;
    Ok(EncodedRichRequest { command, body })
}
