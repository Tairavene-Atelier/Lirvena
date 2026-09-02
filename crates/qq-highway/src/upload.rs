use std::time::Duration;

use md5::{Digest, Md5};
use reqwest::header::{ACCEPT_ENCODING, CONNECTION, HeaderValue, USER_AGENT};

use crate::{
    HighwayEndpoint, HighwayError, HighwaySession, UploadBlock, decode_upload_response,
    encode_upload_block,
};

const DEFAULT_CHUNK_BYTES: usize = 256 * 1024;
const MIN_CHUNK_BYTES: usize = 64 * 1024;
const MAX_CHUNK_BYTES: usize = 4 * 1024 * 1024;
const MAX_RESPONSE_BYTES: usize = 4 * 1024 * 1024 + 64 * 1024 + 10;
const USER_AGENT_VALUE: &str = "Mozilla/5.0 (compatible; MSIE 10.0; Windows NT 6.2)";

/// In-memory authenticated identity required for one upload.
pub struct UploadIdentity<'a> {
    /// QQ account number associated with the login session.
    pub uin: u64,
    /// Profile-provided application identifier.
    pub app_id: u32,
    /// Profile-provided sub-application identifier.
    pub sub_app_id: u32,
    /// Login signature retained only by the authenticated account runtime.
    pub login_signature: &'a [u8],
}

/// Summary of a completely acknowledged object upload.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UploadReceipt {
    bytes_uploaded: u64,
    blocks_uploaded: u32,
}

impl UploadReceipt {
    /// Returns the total acknowledged object size.
    #[must_use]
    pub const fn bytes_uploaded(self) -> u64 {
        self.bytes_uploaded
    }

    /// Returns the number of sequentially acknowledged blocks.
    #[must_use]
    pub const fn blocks_uploaded(self) -> u32 {
        self.blocks_uploaded
    }
}

/// Low-concurrency HTTP client for authenticated QQ Highway uploads.
#[derive(Clone)]
pub struct HighwayClient {
    client: reqwest::Client,
    chunk_bytes: usize,
}

impl HighwayClient {
    /// Builds a client with a 30-second request timeout, no redirects, and a
    /// conservative 256 KiB sequential block size.
    ///
    /// # Errors
    ///
    /// Returns an error when the HTTP client cannot be constructed.
    pub fn new() -> Result<Self, HighwayError> {
        Self::with_chunk_bytes(DEFAULT_CHUNK_BYTES)
    }

    /// Builds a client with a bounded caller-selected sequential block size.
    ///
    /// # Errors
    ///
    /// Returns an error when the block size is outside 64 KiB through 4 MiB,
    /// or when the HTTP client cannot be constructed.
    pub fn with_chunk_bytes(chunk_bytes: usize) -> Result<Self, HighwayError> {
        if !(MIN_CHUNK_BYTES..=MAX_CHUNK_BYTES).contains(&chunk_bytes) {
            return Err(HighwayError::InvalidInput);
        }
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|_| HighwayError::Transport)?;
        Ok(Self {
            client,
            chunk_bytes,
        })
    }

    /// Uploads an in-memory object sequentially, retrying each block only
    /// across the bounded endpoints authenticated by the QQ session response.
    ///
    /// # Errors
    ///
    /// Returns an error when inputs are invalid, no endpoint acknowledges a
    /// block, or QQ returns a malformed or nonzero response.
    pub async fn upload(
        &self,
        session: &HighwaySession,
        identity: &UploadIdentity<'_>,
        command_id: u32,
        extension: &[u8],
        bytes: &[u8],
    ) -> Result<UploadReceipt, HighwayError> {
        if bytes.is_empty() || session.endpoints().is_empty() {
            return Err(HighwayError::InvalidInput);
        }
        let ticket = session.ticket();
        if ticket.is_empty() {
            return Err(HighwayError::InvalidInput);
        }
        let file_md5: [u8; 16] = Md5::digest(bytes).into();
        let file_size = u64::try_from(bytes.len()).map_err(|_| HighwayError::InvalidInput)?;
        let mut blocks_uploaded = 0_u32;
        for (index, body) in bytes.chunks(self.chunk_bytes).enumerate() {
            let sequence = u32::try_from(index)
                .ok()
                .and_then(|value| value.checked_add(1))
                .ok_or(HighwayError::InvalidInput)?;
            let offset_usize = index
                .checked_mul(self.chunk_bytes)
                .ok_or(HighwayError::InvalidInput)?;
            let offset = u64::try_from(offset_usize).map_err(|_| HighwayError::InvalidInput)?;
            let frame = encode_upload_block(&UploadBlock {
                uin: identity.uin,
                sequence,
                sub_app_id: identity.sub_app_id,
                app_id: identity.app_id,
                command_id,
                file_size,
                offset,
                ticket,
                file_md5: &file_md5,
                extension,
                body,
                login_signature: identity.login_signature,
            })?;
            self.send_block(session.endpoints(), identity.uin, frame)
                .await?;
            blocks_uploaded = blocks_uploaded
                .checked_add(1)
                .ok_or(HighwayError::InvalidInput)?;
        }
        Ok(UploadReceipt {
            bytes_uploaded: file_size,
            blocks_uploaded,
        })
    }

    async fn send_block(
        &self,
        endpoints: &[HighwayEndpoint],
        uin: u64,
        frame: Vec<u8>,
    ) -> Result<(), HighwayError> {
        for endpoint in endpoints {
            let url = format!(
                "http://{}:{}/cgi-bin/httpconn?htcmd=0x6FF0087&uin={uin}",
                endpoint.address(),
                endpoint.port()
            );
            let response = match self
                .client
                .post(url)
                .header(CONNECTION, HeaderValue::from_static("keep-alive"))
                .header(ACCEPT_ENCODING, HeaderValue::from_static("identity"))
                .header(USER_AGENT, HeaderValue::from_static(USER_AGENT_VALUE))
                .body(frame.clone())
                .send()
                .await
            {
                Ok(response) if response.status().is_success() => response,
                Ok(_) | Err(_) => continue,
            };
            let Ok(response_bytes) = response.bytes().await else {
                continue;
            };
            if response_bytes.len() > MAX_RESPONSE_BYTES {
                continue;
            }
            match decode_upload_response(&response_bytes) {
                Ok(_) => return Ok(()),
                Err(HighwayError::RemoteRejected) => return Err(HighwayError::RemoteRejected),
                Err(_) => {}
            }
        }
        Err(HighwayError::Transport)
    }
}
