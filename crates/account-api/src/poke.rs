use crate::{AccountIdentity, EventHubError};

/// Adapter-neutral scope of a poke notice.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResolvedPokeScope {
    /// A direct-contact poke.
    Friend,
    /// A group poke.
    Group(u64),
}

/// One authenticated poke ready for adapters.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedPoke {
    account: AccountIdentity,
    scope: ResolvedPokeScope,
    operator_id: u64,
    target_id: u64,
    action: String,
    suffix: String,
    image_url: String,
    occurred_at: u64,
}

impl ResolvedPoke {
    /// Creates one bounded poke event.
    ///
    /// # Errors
    ///
    /// Returns an error for missing identities or unsafe descriptive text.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        account: AccountIdentity,
        scope: ResolvedPokeScope,
        operator_id: u64,
        target_id: u64,
        action: String,
        suffix: String,
        image_url: String,
        occurred_at: u64,
    ) -> Result<Self, EventHubError> {
        let valid_scope = !matches!(scope, ResolvedPokeScope::Group(0));
        let valid_text = [&action, &suffix, &image_url]
            .into_iter()
            .all(|value| value.len() <= 2048 && !value.contains('\0'));
        if !valid_scope || operator_id == 0 || target_id == 0 || !valid_text {
            return Err(EventHubError::InvalidEvent);
        }
        Ok(Self {
            account,
            scope,
            operator_id,
            target_id,
            action,
            suffix,
            image_url,
            occurred_at,
        })
    }
    /// Returns the receiving account.
    #[must_use]
    pub const fn account(&self) -> &AccountIdentity {
        &self.account
    }
    /// Returns the conversation scope.
    #[must_use]
    pub const fn scope(&self) -> ResolvedPokeScope {
        self.scope
    }
    /// Returns the numeric operator identifier.
    #[must_use]
    pub const fn operator_id(&self) -> u64 {
        self.operator_id
    }
    /// Returns the numeric target identifier.
    #[must_use]
    pub const fn target_id(&self) -> u64 {
        self.target_id
    }
    /// Returns QQ's action description.
    #[must_use]
    pub fn action(&self) -> &str {
        &self.action
    }
    /// Returns QQ's suffix description.
    #[must_use]
    pub fn suffix(&self) -> &str {
        &self.suffix
    }
    /// Returns QQ's action image URL.
    #[must_use]
    pub fn image_url(&self) -> &str {
        &self.image_url
    }
    /// Returns the QQ-supplied Unix event time.
    #[must_use]
    pub const fn occurred_at(&self) -> u64 {
        self.occurred_at
    }
}
