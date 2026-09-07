use crate::{AccountIdentity, EventHubError};

/// One authenticated private-file notice with a QQ-issued download URL.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedPrivateFile {
    account: AccountIdentity,
    sender_id: u64,
    file_id: String,
    name: String,
    size: u64,
    url: String,
    hash: String,
    occurred_at: u64,
}

/// One authenticated group-file notice with a QQ-issued download URL.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedGroupFile {
    account: AccountIdentity,
    group_id: u64,
    sender_id: u64,
    bus_id: u32,
    file_id: String,
    name: String,
    size: u64,
    url: String,
    occurred_at: u64,
}

impl ResolvedGroupFile {
    /// Creates one adapter-neutral group-file notice.
    ///
    /// # Errors
    ///
    /// Returns an error for missing identities, metadata, or unsafe text.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        account: AccountIdentity,
        group_id: u64,
        sender_id: u64,
        bus_id: u32,
        file_id: String,
        name: String,
        size: u64,
        url: String,
        occurred_at: u64,
    ) -> Result<Self, EventHubError> {
        if group_id == 0
            || sender_id == 0
            || bus_id == 0
            || size == 0
            || [&file_id, &name, &url]
                .into_iter()
                .any(|value| value.is_empty() || value.len() > 8192 || value.contains('\0'))
        {
            return Err(EventHubError::InvalidEvent);
        }
        Ok(Self {
            account,
            group_id,
            sender_id,
            bus_id,
            file_id,
            name,
            size,
            url,
            occurred_at,
        })
    }
    /// Returns the receiving account.
    #[must_use]
    pub const fn account(&self) -> &AccountIdentity {
        &self.account
    }
    /// Returns the QQ group identifier.
    #[must_use]
    pub const fn group_id(&self) -> u64 {
        self.group_id
    }
    /// Returns the numeric sender identifier.
    #[must_use]
    pub const fn sender_id(&self) -> u64 {
        self.sender_id
    }
    /// Returns the QQ file bus identifier.
    #[must_use]
    pub const fn bus_id(&self) -> u32 {
        self.bus_id
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
    /// Returns the QQ-issued download URL.
    #[must_use]
    pub fn url(&self) -> &str {
        &self.url
    }
    /// Returns the QQ-supplied Unix event time.
    #[must_use]
    pub const fn occurred_at(&self) -> u64 {
        self.occurred_at
    }
}

impl ResolvedPrivateFile {
    /// Creates one adapter-neutral private-file notice.
    ///
    /// # Errors
    ///
    /// Returns an error for missing identities, tokens, URL, or unsafe text.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        account: AccountIdentity,
        sender_id: u64,
        file_id: String,
        name: String,
        size: u64,
        url: String,
        hash: String,
        occurred_at: u64,
    ) -> Result<Self, EventHubError> {
        let values = [&file_id, &name, &url, &hash];
        if sender_id == 0
            || size == 0
            || values
                .into_iter()
                .any(|value| value.is_empty() || value.len() > 8192 || value.contains('\0'))
        {
            return Err(EventHubError::InvalidEvent);
        }
        Ok(Self {
            account,
            sender_id,
            file_id,
            name,
            size,
            url,
            hash,
            occurred_at,
        })
    }
    /// Returns the receiving account.
    #[must_use]
    pub const fn account(&self) -> &AccountIdentity {
        &self.account
    }
    /// Returns the numeric sender identifier.
    #[must_use]
    pub const fn sender_id(&self) -> u64 {
        self.sender_id
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
    /// Returns the QQ-issued download URL.
    #[must_use]
    pub fn url(&self) -> &str {
        &self.url
    }
    /// Returns the QQ file hash token.
    #[must_use]
    pub fn hash(&self) -> &str {
        &self.hash
    }
    /// Returns the QQ-supplied Unix event time.
    #[must_use]
    pub const fn occurred_at(&self) -> u64 {
        self.occurred_at
    }
}
