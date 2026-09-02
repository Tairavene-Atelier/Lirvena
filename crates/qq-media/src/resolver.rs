use std::collections::BTreeSet;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::time::Duration;

use base64::Engine;
use futures_util::StreamExt;
use tokio::io::AsyncReadExt;
use url::Url;

use crate::{MediaError, MediaObject, MediaReference, MediaSourceKind};

const HARD_MAX_MEDIA_BYTES: usize = 256 * 1024 * 1024;
const MAX_ALLOWED_ROOTS: usize = 16;
const MAX_REMOTE_HOSTS: usize = 64;
const MAX_RESOLVED_ADDRESSES: usize = 16;

/// Explicit remote-media policy; absence disables remote acquisition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RemoteMediaPolicy {
    allowed_hosts: BTreeSet<String>,
    timeout: Duration,
}

impl RemoteMediaPolicy {
    /// Builds an HTTPS allowlist.
    ///
    /// # Errors
    ///
    /// Returns an error for an empty/excessive host set, malformed lowercase DNS names, or a
    /// timeout outside one second through two minutes.
    pub fn new(
        allowed_hosts: impl IntoIterator<Item = String>,
        timeout: Duration,
    ) -> Result<Self, MediaError> {
        let allowed_hosts = allowed_hosts.into_iter().collect::<BTreeSet<_>>();
        if allowed_hosts.is_empty()
            || allowed_hosts.len() > MAX_REMOTE_HOSTS
            || !(Duration::from_secs(1)..=Duration::from_mins(2)).contains(&timeout)
            || allowed_hosts.iter().any(|host| !valid_host(host))
        {
            return Err(MediaError::Configuration);
        }
        Ok(Self {
            allowed_hosts,
            timeout,
        })
    }
}

/// Bounded acquisition policy shared by all adapters.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MediaPolicy {
    roots: Box<[PathBuf]>,
    cache_directory: Option<PathBuf>,
    maximum_bytes: usize,
    remote: Option<RemoteMediaPolicy>,
}

impl MediaPolicy {
    /// Creates a policy from selected roots and optional cache/remote configuration.
    ///
    /// # Errors
    ///
    /// Returns an error for excessive roots, empty paths, or a byte bound outside the compiled
    /// maximum.
    pub fn new(
        roots: Vec<PathBuf>,
        cache_directory: Option<PathBuf>,
        maximum_bytes: usize,
        remote: Option<RemoteMediaPolicy>,
    ) -> Result<Self, MediaError> {
        if roots.len() > MAX_ALLOWED_ROOTS
            || roots.iter().any(|path| path.as_os_str().is_empty())
            || cache_directory
                .as_ref()
                .is_some_and(|path| path.as_os_str().is_empty())
            || maximum_bytes == 0
            || maximum_bytes > HARD_MAX_MEDIA_BYTES
        {
            return Err(MediaError::Configuration);
        }
        Ok(Self {
            roots: roots.into_boxed_slice(),
            cache_directory,
            maximum_bytes,
            remote,
        })
    }
}

/// Stateless media resolver applying one shared policy.
#[derive(Clone, Debug)]
pub struct MediaResolver {
    policy: MediaPolicy,
}

impl MediaResolver {
    /// Creates a resolver without performing filesystem or network I/O.
    #[must_use]
    pub const fn new(policy: MediaPolicy) -> Self {
        Self { policy }
    }

    /// Acquires one bounded media object.
    ///
    /// # Errors
    ///
    /// Returns an explicit policy, size, I/O, or remote destination error.
    pub async fn resolve(&self, reference: &MediaReference) -> Result<MediaObject, MediaError> {
        match reference {
            MediaReference::Local(path) => self.local(path, MediaSourceKind::LocalFile).await,
            MediaReference::Cache(key) => self.cache(key).await,
            MediaReference::InlineBase64(value) => self.inline(value),
            MediaReference::Remote(url) => self.remote(url).await,
        }
    }

    async fn cache(&self, key: &str) -> Result<MediaObject, MediaError> {
        let directory = self
            .policy
            .cache_directory
            .as_ref()
            .ok_or(MediaError::ReferenceRejected)?;
        self.local(&directory.join(key), MediaSourceKind::Cache)
            .await
    }

    async fn local(&self, path: &Path, source: MediaSourceKind) -> Result<MediaObject, MediaError> {
        let canonical = tokio::fs::canonicalize(path)
            .await
            .map_err(|_error| MediaError::LocalFileRejected)?;
        let allowed = match source {
            MediaSourceKind::Cache => {
                let root = self
                    .policy
                    .cache_directory
                    .as_ref()
                    .ok_or(MediaError::ReferenceRejected)?;
                tokio::fs::canonicalize(root)
                    .await
                    .map_err(|_error| MediaError::LocalFileRejected)?
            }
            _ => canonical_root(&self.policy.roots, &canonical).await?,
        };
        if !canonical.starts_with(&allowed) {
            return Err(MediaError::LocalFileRejected);
        }
        let metadata = tokio::fs::metadata(&canonical)
            .await
            .map_err(|_error| MediaError::LocalFileRejected)?;
        if !metadata.is_file()
            || metadata.len()
                > u64::try_from(self.policy.maximum_bytes)
                    .map_err(|_error| MediaError::SizeLimit)?
        {
            return Err(MediaError::LocalFileRejected);
        }
        let file = tokio::fs::File::open(canonical)
            .await
            .map_err(|_error| MediaError::Io)?;
        let mut bytes = Vec::with_capacity(
            usize::try_from(metadata.len()).map_err(|_error| MediaError::SizeLimit)?,
        );
        file.take(
            u64::try_from(self.policy.maximum_bytes + 1).map_err(|_error| MediaError::SizeLimit)?,
        )
        .read_to_end(&mut bytes)
        .await
        .map_err(|_error| MediaError::Io)?;
        if bytes.len() > self.policy.maximum_bytes {
            return Err(MediaError::SizeLimit);
        }
        Ok(MediaObject::new(bytes, source))
    }

    fn inline(&self, value: &str) -> Result<MediaObject, MediaError> {
        let estimated = value.len().saturating_mul(3) / 4;
        if estimated > self.policy.maximum_bytes {
            return Err(MediaError::SizeLimit);
        }
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(value)
            .map_err(|_error| MediaError::ReferenceRejected)?;
        if bytes.is_empty() || bytes.len() > self.policy.maximum_bytes {
            return Err(MediaError::SizeLimit);
        }
        Ok(MediaObject::new(bytes, MediaSourceKind::InlineBase64))
    }

    async fn remote(&self, url: &Url) -> Result<MediaObject, MediaError> {
        let policy = self
            .policy
            .remote
            .as_ref()
            .ok_or(MediaError::ReferenceRejected)?;
        if url.scheme() != "https" || !url.username().is_empty() || url.password().is_some() {
            return Err(MediaError::RemoteRejected);
        }
        let host = url.host_str().ok_or(MediaError::RemoteRejected)?;
        if !policy.allowed_hosts.contains(host) {
            return Err(MediaError::RemoteRejected);
        }
        let port = url
            .port_or_known_default()
            .ok_or(MediaError::RemoteRejected)?;
        let addresses = resolve_public(host, port).await?;
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .timeout(policy.timeout)
            .resolve_to_addrs(host, &addresses)
            .build()
            .map_err(|_error| MediaError::RemoteRejected)?;
        let response = client
            .get(url.clone())
            .send()
            .await
            .map_err(|_error| MediaError::RemoteRejected)?;
        if !response.status().is_success()
            || response
                .content_length()
                .is_some_and(|length| length > self.policy.maximum_bytes as u64)
        {
            return Err(MediaError::RemoteRejected);
        }
        let mut bytes = Vec::new();
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|_error| MediaError::RemoteRejected)?;
            if bytes.len().saturating_add(chunk.len()) > self.policy.maximum_bytes {
                return Err(MediaError::SizeLimit);
            }
            bytes.extend_from_slice(&chunk);
        }
        if bytes.is_empty() {
            return Err(MediaError::RemoteRejected);
        }
        Ok(MediaObject::new(bytes, MediaSourceKind::RemoteHttps))
    }
}

async fn canonical_root(roots: &[PathBuf], path: &Path) -> Result<PathBuf, MediaError> {
    for root in roots {
        let canonical = tokio::fs::canonicalize(root)
            .await
            .map_err(|_error| MediaError::LocalFileRejected)?;
        if path.starts_with(&canonical) {
            return Ok(canonical);
        }
    }
    Err(MediaError::LocalFileRejected)
}

async fn resolve_public(host: &str, port: u16) -> Result<Vec<SocketAddr>, MediaError> {
    let addresses = tokio::net::lookup_host((host, port))
        .await
        .map_err(|_error| MediaError::RemoteRejected)?
        .take(MAX_RESOLVED_ADDRESSES + 1)
        .collect::<Vec<_>>();
    if addresses.is_empty()
        || addresses.len() > MAX_RESOLVED_ADDRESSES
        || addresses.iter().any(|address| !is_public(address.ip()))
    {
        return Err(MediaError::RemoteRejected);
    }
    Ok(addresses)
}

fn is_public(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(value) => public_v4(value),
        IpAddr::V6(value) => public_v6(value),
    }
}

fn public_v4(value: Ipv4Addr) -> bool {
    let octets = value.octets();
    !(value.is_unspecified()
        || value.is_loopback()
        || value.is_private()
        || value.is_link_local()
        || value.is_multicast()
        || value.is_broadcast()
        || octets[0] == 0
        || octets[0] >= 240
        || (octets[0] == 100 && (64..=127).contains(&octets[1]))
        || (octets[0] == 192 && octets[1] == 0 && octets[2] == 2)
        || (octets[0] == 198 && (octets[1] == 18 || octets[1] == 19))
        || (octets[0] == 198 && octets[1] == 51 && octets[2] == 100)
        || (octets[0] == 203 && octets[1] == 0 && octets[2] == 113))
}

fn public_v6(value: Ipv6Addr) -> bool {
    let segments = value.segments();
    !(value.is_unspecified()
        || value.is_loopback()
        || value.is_multicast()
        || (segments[0] & 0xfe00) == 0xfc00
        || (segments[0] & 0xffc0) == 0xfe80
        || (segments[0] == 0x2001 && segments[1] == 0x0db8))
}

fn valid_host(host: &str) -> bool {
    !host.is_empty()
        && host.len() <= 253
        && host == host.to_ascii_lowercase()
        && !host.starts_with('.')
        && !host.ends_with('.')
        && host.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'-')
        })
}

#[cfg(test)]
mod tests {
    use super::{IpAddr, Ipv4Addr, Ipv6Addr, is_public};

    #[test]
    fn remote_destinations_reject_local_and_documentation_ranges() {
        assert!(!is_public(IpAddr::V4(Ipv4Addr::LOCALHOST)));
        assert!(!is_public(IpAddr::V4(Ipv4Addr::new(192, 0, 2, 1))));
        assert!(!is_public(IpAddr::V6(Ipv6Addr::LOCALHOST)));
        assert!(is_public(IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1))));
        assert!(is_public(IpAddr::V6(Ipv6Addr::new(
            0x2606, 0x4700, 0x4700, 0, 0, 0, 0, 0x1111,
        ))));
    }
}
