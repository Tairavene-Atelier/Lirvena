use prost::Message;

use crate::MediaError;
use crate::image_proto::{
    HighwayAddress, HighwayDomain, HighwayExtension, HighwayHashes, HighwayNetwork, IndexNode,
    RawMessageBody, RawMessageInfo, RichResponse, UploadResponse,
};

const MAX_RESPONSE_BYTES: usize = 2 * 1024 * 1024;
const MAX_MESSAGE_INFO_BYTES: usize = 1024 * 1024;
const MAX_COMPATIBILITY_BYTES: usize = 512 * 1024;
const MAX_UPLOAD_KEY_BYTES: usize = 4 * 1024;
const MAX_MESSAGE_BODIES: usize = 16;
const MAX_NETWORK_ADDRESSES: usize = 32;
const HIGHWAY_BLOCK_BYTES: u32 = 1024 * 1024;

/// Tencent-created rich-media message material and optional upload continuation.
#[derive(Clone, Eq, PartialEq)]
pub struct RichMediaUploadPlan {
    message_info: Vec<u8>,
    compatibility_message: Vec<u8>,
    highway_extension: Option<Vec<u8>>,
    command_id: u32,
}

impl RichMediaUploadPlan {
    pub(crate) fn parse(input: &[u8], command_id: u32) -> Result<Self, MediaError> {
        if input.len() > MAX_RESPONSE_BYTES {
            return Err(MediaError::RemoteRejected);
        }
        let outer =
            qq_wire::decode_oidb_response(input).map_err(|_error| MediaError::RemoteRejected)?;
        if outer.error_code() != 0 {
            return Err(MediaError::RemoteRejected);
        }
        let response =
            RichResponse::decode(outer.body()).map_err(|_error| MediaError::RemoteRejected)?;
        if response
            .head
            .as_ref()
            .is_some_and(|head| head.return_code != 0)
        {
            return Err(MediaError::RemoteRejected);
        }
        let upload = response.upload.ok_or(MediaError::RemoteRejected)?;
        validate_response(&upload)?;
        let highway_extension = if upload.upload_key.is_empty() {
            None
        } else {
            Some(build_highway_extension(&upload)?)
        };
        Ok(Self {
            message_info: upload.message_info,
            compatibility_message: upload.compatibility_message,
            highway_extension,
            command_id,
        })
    }

    /// Returns Tencent-created modern message information unchanged.
    #[must_use]
    pub fn message_info(&self) -> &[u8] {
        &self.message_info
    }

    /// Returns Tencent-created optional legacy compatibility material unchanged.
    #[must_use]
    pub fn compatibility_message(&self) -> &[u8] {
        &self.compatibility_message
    }

    /// Returns the upload extension, or `None` when fast upload already completed.
    #[must_use]
    pub fn highway_extension(&self) -> Option<&[u8]> {
        self.highway_extension.as_deref()
    }

    /// Returns the audited Highway media command identifier.
    #[must_use]
    pub const fn command_id(&self) -> u32 {
        self.command_id
    }

    /// Consumes the plan and returns its message material.
    #[must_use]
    pub fn into_message_material(self) -> (Vec<u8>, Vec<u8>) {
        (self.message_info, self.compatibility_message)
    }
}

impl core::fmt::Debug for RichMediaUploadPlan {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("RichMediaUploadPlan")
            .field("message_info_bytes", &self.message_info.len())
            .field("compatibility_bytes", &self.compatibility_message.len())
            .field("highway_required", &self.highway_extension.is_some())
            .field("command_id", &self.command_id)
            .finish()
    }
}

fn validate_response(upload: &UploadResponse) -> Result<(), MediaError> {
    if upload.message_info.is_empty()
        || upload.message_info.len() > MAX_MESSAGE_INFO_BYTES
        || upload.compatibility_message.len() > MAX_COMPATIBILITY_BYTES
        || upload.upload_key.len() > MAX_UPLOAD_KEY_BYTES
        || upload.ipv4.len() > MAX_NETWORK_ADDRESSES
        || upload.upload_key.chars().any(char::is_control)
    {
        return Err(MediaError::RemoteRejected);
    }
    Ok(())
}

fn build_highway_extension(upload: &UploadResponse) -> Result<Vec<u8>, MediaError> {
    let message = RawMessageInfo::decode(upload.message_info.as_slice())
        .map_err(|_error| MediaError::RemoteRejected)?;
    if message.bodies.is_empty() || message.bodies.len() > MAX_MESSAGE_BODIES {
        return Err(MediaError::RemoteRejected);
    }
    let body = RawMessageBody::decode(message.bodies[0].as_slice())
        .map_err(|_error| MediaError::RemoteRejected)?;
    let index =
        IndexNode::decode(body.index.as_slice()).map_err(|_error| MediaError::RemoteRejected)?;
    let info = index.info.ok_or(MediaError::RemoteRejected)?;
    let sha1 = crate::image::decode_hex::<20>(&info.sha1)?;
    if index.uuid.is_empty() || index.uuid.len() > 512 || index.uuid.chars().any(char::is_control) {
        return Err(MediaError::RemoteRejected);
    }
    let addresses = upload
        .ipv4
        .iter()
        .filter_map(|value| {
            let port = u16::try_from(value.external_port).ok()?;
            if port == 0 {
                return None;
            }
            Some(HighwayAddress {
                domain: Some(HighwayDomain {
                    enabled: true,
                    address: std::net::Ipv4Addr::from(value.external_address.to_le_bytes())
                        .to_string(),
                }),
                port: u32::from(port),
            })
        })
        .collect();
    Ok(HighwayExtension {
        uuid: index.uuid,
        upload_key: upload.upload_key.clone(),
        network: Some(HighwayNetwork { addresses }),
        message_bodies: message.bodies,
        block_size: HIGHWAY_BLOCK_BYTES,
        hashes: Some(HighwayHashes {
            sha1: vec![sha1.to_vec()],
        }),
    }
    .encode_to_vec())
}
