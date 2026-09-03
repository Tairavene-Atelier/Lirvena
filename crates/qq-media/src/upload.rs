use prost::Message;
use sha1::{Digest as _, Sha1};

use crate::MediaError;
use crate::image_proto::{
    HighwayAddress, HighwayDomain, HighwayExtension, HighwayHashes, HighwayNetwork, IndexNode,
    Ipv4, RawMessageBody, RawMessageInfo, RichResponse, UploadResponse,
};

const MAX_RESPONSE_BYTES: usize = 2 * 1024 * 1024;
const MAX_MESSAGE_INFO_BYTES: usize = 1024 * 1024;
const MAX_COMPATIBILITY_BYTES: usize = 512 * 1024;
const MAX_UPLOAD_KEY_BYTES: usize = 4 * 1024;
const MAX_MESSAGE_BODIES: usize = 16;
const MAX_NETWORK_ADDRESSES: usize = 32;
const MAX_SUB_FILES: usize = 8;
const HIGHWAY_BLOCK_BYTES: u32 = 1024 * 1024;
const HIGHWAY_BLOCK_SIZE: usize = 1024 * 1024;

/// Tencent-created rich-media message material and optional upload continuation.
#[derive(Clone, Eq, PartialEq)]
pub struct RichMediaUploadPlan {
    message_info: Vec<u8>,
    compatibility_message: Vec<u8>,
    uploads: Vec<RichMediaUpload>,
}

/// One required QQ Highway continuation bound to a negotiated file index.
#[derive(Clone, Eq, PartialEq)]
pub struct RichMediaUpload {
    file_index: usize,
    command_id: u32,
    extension: Vec<u8>,
    cumulative_hashes: bool,
}

impl RichMediaUploadPlan {
    pub(crate) fn parse(input: &[u8], command_id: u32) -> Result<Self, MediaError> {
        Self::parse_with_sub_files(input, (command_id, false), &[])
    }

    pub(crate) fn parse_with_sub_files(
        input: &[u8],
        main: (u32, bool),
        sub_files: &[(u32, u32, bool)],
    ) -> Result<Self, MediaError> {
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
        if upload.sub_files.len() > MAX_SUB_FILES || upload.sub_files.len() != sub_files.len() {
            return Err(MediaError::RemoteRejected);
        }
        let mut uploads = Vec::with_capacity(1 + sub_files.len());
        if !upload.upload_key.is_empty() {
            uploads.push(RichMediaUpload {
                file_index: 0,
                command_id: main.0,
                extension: build_highway_extension(
                    &upload.message_info,
                    &upload.upload_key,
                    &upload.ipv4,
                    0,
                )?,
                cumulative_hashes: main.1,
            });
        }
        for (position, (sub_file_type, sub_command_id, cumulative_hashes)) in
            sub_files.iter().enumerate()
        {
            let negotiated = upload
                .sub_files
                .iter()
                .find(|candidate| candidate.sub_file_type == *sub_file_type)
                .ok_or(MediaError::RemoteRejected)?;
            if upload
                .sub_files
                .iter()
                .filter(|candidate| candidate.sub_file_type == *sub_file_type)
                .count()
                != 1
            {
                return Err(MediaError::RemoteRejected);
            }
            validate_key_and_addresses(&negotiated.upload_key, &negotiated.ipv4)?;
            if !negotiated.upload_key.is_empty() {
                uploads.push(RichMediaUpload {
                    file_index: position + 1,
                    command_id: *sub_command_id,
                    extension: build_highway_extension(
                        &upload.message_info,
                        &negotiated.upload_key,
                        &negotiated.ipv4,
                        position + 1,
                    )?,
                    cumulative_hashes: *cumulative_hashes,
                });
            }
        }
        Ok(Self {
            message_info: upload.message_info,
            compatibility_message: upload.compatibility_message,
            uploads,
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

    /// Returns the ordered Highway continuations still required after fast-upload negotiation.
    #[must_use]
    pub fn uploads(&self) -> &[RichMediaUpload] {
        &self.uploads
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
            .field("highway_uploads", &self.uploads.len())
            .finish()
    }
}

impl RichMediaUpload {
    /// Returns the zero-based input file index negotiated for this upload.
    #[must_use]
    pub const fn file_index(&self) -> usize {
        self.file_index
    }

    /// Returns the audited Highway media command identifier.
    #[must_use]
    pub const fn command_id(&self) -> u32 {
        self.command_id
    }

    /// Returns the opaque QQ Highway extension with any audited stream hash projection applied.
    ///
    /// # Errors
    ///
    /// Returns an error if the negotiated extension can no longer be decoded.
    pub fn extension_for(&self, bytes: &[u8]) -> Result<Vec<u8>, MediaError> {
        if !self.cumulative_hashes {
            return Ok(self.extension.clone());
        }
        let mut extension = HighwayExtension::decode(self.extension.as_slice())
            .map_err(|_error| MediaError::RemoteRejected)?;
        extension.hashes = Some(HighwayHashes {
            sha1: cumulative_sha1(bytes),
        });
        Ok(extension.encode_to_vec())
    }
}

fn cumulative_sha1(bytes: &[u8]) -> Vec<Vec<u8>> {
    let mut hasher = Sha1::new();
    let mut digests = Vec::with_capacity(bytes.len() / HIGHWAY_BLOCK_SIZE + 1);
    for chunk in bytes.chunks(HIGHWAY_BLOCK_SIZE) {
        hasher.update(chunk);
        digests.push(hasher.clone().finalize().to_vec());
    }
    if !bytes.is_empty() && bytes.len().is_multiple_of(HIGHWAY_BLOCK_SIZE) {
        digests.push(hasher.finalize().to_vec());
    }
    digests
}

fn validate_response(upload: &UploadResponse) -> Result<(), MediaError> {
    if upload.message_info.is_empty()
        || upload.message_info.len() > MAX_MESSAGE_INFO_BYTES
        || upload.compatibility_message.len() > MAX_COMPATIBILITY_BYTES
    {
        return Err(MediaError::RemoteRejected);
    }
    validate_key_and_addresses(&upload.upload_key, &upload.ipv4)
}

fn validate_key_and_addresses(upload_key: &str, ipv4: &[Ipv4]) -> Result<(), MediaError> {
    if upload_key.len() > MAX_UPLOAD_KEY_BYTES
        || ipv4.len() > MAX_NETWORK_ADDRESSES
        || upload_key.chars().any(char::is_control)
    {
        return Err(MediaError::RemoteRejected);
    }
    Ok(())
}

fn build_highway_extension(
    message_info: &[u8],
    upload_key: &str,
    ipv4: &[Ipv4],
    body_index: usize,
) -> Result<Vec<u8>, MediaError> {
    let message =
        RawMessageInfo::decode(message_info).map_err(|_error| MediaError::RemoteRejected)?;
    if message.bodies.is_empty()
        || message.bodies.len() > MAX_MESSAGE_BODIES
        || body_index >= message.bodies.len()
    {
        return Err(MediaError::RemoteRejected);
    }
    let body = RawMessageBody::decode(message.bodies[body_index].as_slice())
        .map_err(|_error| MediaError::RemoteRejected)?;
    let index =
        IndexNode::decode(body.index.as_slice()).map_err(|_error| MediaError::RemoteRejected)?;
    let info = index.info.ok_or(MediaError::RemoteRejected)?;
    let sha1 = crate::image::decode_hex::<20>(&info.sha1)?;
    if index.uuid.is_empty() || index.uuid.len() > 512 || index.uuid.chars().any(char::is_control) {
        return Err(MediaError::RemoteRejected);
    }
    let addresses = ipv4
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
        upload_key: upload_key.to_owned(),
        network: Some(HighwayNetwork { addresses }),
        message_bodies: message.bodies,
        block_size: HIGHWAY_BLOCK_BYTES,
        hashes: Some(HighwayHashes {
            sha1: vec![sha1.to_vec()],
        }),
    }
    .encode_to_vec())
}

#[cfg(test)]
mod tests {
    use super::{HIGHWAY_BLOCK_SIZE, cumulative_sha1};

    #[test]
    fn video_stream_hashes_match_the_audited_boundary_behavior() {
        let exact_block = vec![0x5a; HIGHWAY_BLOCK_SIZE];
        let exact_digests = cumulative_sha1(&exact_block);
        assert_eq!(exact_digests.len(), 2);
        assert_eq!(exact_digests[0], exact_digests[1]);

        let mut partial_tail = exact_block;
        partial_tail.push(0xa5);
        let partial_digests = cumulative_sha1(&partial_tail);
        assert_eq!(partial_digests.len(), 2);
        assert_ne!(partial_digests[0], partial_digests[1]);
    }
}
