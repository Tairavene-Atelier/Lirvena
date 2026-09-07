use prost::Message;

use crate::{MessageClass, MessageDecodeError, MessageEnvelope};

const MAX_FILE_NAME_BYTES: usize = 4096;
const MAX_TOKEN_BYTES: usize = 4096;

/// One authenticated incoming private-file descriptor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrivateFileNotice {
    sender_id: u32,
    sender_uid: String,
    file_id: String,
    name: String,
    size: u64,
    hash: String,
}

impl PrivateFileNotice {
    /// Returns the numeric sender identifier.
    #[must_use]
    pub const fn sender_id(&self) -> u32 {
        self.sender_id
    }
    /// Returns the current Linux NT sender UID used for the URL query.
    #[must_use]
    pub fn sender_uid(&self) -> &str {
        &self.sender_uid
    }
    /// Returns the QQ file identifier.
    #[must_use]
    pub fn file_id(&self) -> &str {
        &self.file_id
    }
    /// Returns the sender-provided file name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }
    /// Returns the file size in bytes.
    #[must_use]
    pub const fn size(&self) -> u64 {
        self.size
    }
    /// Returns the QQ file hash token.
    #[must_use]
    pub fn hash(&self) -> &str {
        &self.hash
    }
}

/// Decodes the frozen-52194 private-file message descriptor.
///
/// # Errors
///
/// Returns an error for missing identities, malformed metadata, unsafe text, or invalid size.
pub fn decode_private_file_notice(
    envelope: &MessageEnvelope,
) -> Result<Option<PrivateFileNotice>, MessageDecodeError> {
    if envelope.class() != MessageClass::PrivateFile {
        return Ok(None);
    }
    let route = envelope.route();
    let content = envelope.payload().content().ok_or(MessageDecodeError)?;
    let file = FileExtraWire::decode(content)
        .map_err(|_error| MessageDecodeError)?
        .file
        .ok_or(MessageDecodeError)?;
    let sender_uid = route.from_uid.clone().ok_or(MessageDecodeError)?;
    let size = u64::try_from(file.size).map_err(|_error| MessageDecodeError)?;
    if route.from_uin == 0
        || sender_uid.is_empty()
        || sender_uid.len() > 128
        || sender_uid.chars().any(char::is_control)
        || file.id.is_empty()
        || file.id.len() > MAX_TOKEN_BYTES
        || file.hash.is_empty()
        || file.hash.len() > MAX_TOKEN_BYTES
        || file.name.is_empty()
        || file.name.len() > MAX_FILE_NAME_BYTES
        || [&file.id, &file.hash, &file.name]
            .into_iter()
            .any(|value| value.contains('\0'))
        || file.md5.is_empty()
        || file.md5.len() > 64
        || size == 0
    {
        return Err(MessageDecodeError);
    }
    Ok(Some(PrivateFileNotice {
        sender_id: route.from_uin,
        sender_uid,
        file_id: file.id,
        name: file.name,
        size,
        hash: file.hash,
    }))
}

#[derive(Clone, PartialEq, Message)]
struct FileExtraWire {
    #[prost(message, optional, tag = "1")]
    file: Option<PrivateFileWire>,
}

#[derive(Clone, PartialEq, Message)]
struct PrivateFileWire {
    #[prost(string, tag = "3")]
    id: String,
    #[prost(bytes = "vec", tag = "4")]
    md5: Vec<u8>,
    #[prost(string, tag = "5")]
    name: String,
    #[prost(int64, tag = "6")]
    size: i64,
    #[prost(string, tag = "57")]
    hash: String,
}
