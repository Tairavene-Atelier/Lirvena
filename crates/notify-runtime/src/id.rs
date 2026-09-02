use crate::NotificationError;

macro_rules! opaque_id {
    ($(#[$attribute:meta])* $name:ident, $length:expr) => {
        $(#[$attribute])*
        #[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name([u8; $length]);

        impl $name {
            /// Exact byte width.
            pub const LENGTH: usize = $length;

            /// Creates an identifier from exact opaque bytes.
            #[must_use]
            pub const fn from_bytes(bytes: [u8; $length]) -> Self {
                Self(bytes)
            }

            /// Borrows the opaque bytes.
            #[must_use]
            pub const fn as_bytes(&self) -> &[u8; $length] {
                &self.0
            }

            pub(crate) fn from_slice(bytes: &[u8]) -> Result<Self, NotificationError> {
                bytes
                    .try_into()
                    .map(Self)
                    .map_err(|_error| NotificationError::Configuration)
            }
        }

        impl core::fmt::Debug for $name {
            fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                formatter.write_str(concat!(stringify!($name), "(<opaque>)"))
            }
        }
    };
}

opaque_id!(
    /// Globally unique notification event identifier.
    EventId,
    16
);
opaque_id!(
    /// Stable local destination identifier independent of its secret configuration.
    DestinationId,
    16
);
opaque_id!(
    /// Stable event-equivalence key used for cooldown suppression.
    DedupeKey,
    32
);

impl EventId {
    /// Generates a random event identifier.
    ///
    /// # Errors
    ///
    /// Returns an error when operating-system randomness is unavailable.
    pub fn random() -> Result<Self, NotificationError> {
        let mut bytes = [0_u8; Self::LENGTH];
        getrandom::fill(&mut bytes).map_err(|_error| NotificationError::Identity)?;
        Ok(Self(bytes))
    }
}

/// Monotonic `SQLite` delivery identifier local to one outbox.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DeliveryId(i64);

impl DeliveryId {
    pub(crate) const fn from_stored(value: i64) -> Result<Self, NotificationError> {
        if value > 0 {
            Ok(Self(value))
        } else {
            Err(NotificationError::Configuration)
        }
    }

    pub(crate) const fn stored(self) -> i64 {
        self.0
    }
}
