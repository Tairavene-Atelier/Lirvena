use core::fmt;

/// Fixed-width identifier conversion failed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FixedBytesLengthError {
    kind: &'static str,
    expected: usize,
    actual: usize,
}

impl FixedBytesLengthError {
    /// Expected byte width.
    #[must_use]
    pub const fn expected(self) -> usize {
        self.expected
    }

    /// Rejected byte width.
    #[must_use]
    pub const fn actual(self) -> usize {
        self.actual
    }
}

impl fmt::Display for FixedBytesLengthError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} requires {} bytes; received {}",
            self.kind, self.expected, self.actual
        )
    }
}

impl std::error::Error for FixedBytesLengthError {}

macro_rules! fixed_bytes {
    ($(#[$attribute:meta])* $name:ident, $length:expr) => {
        $(#[$attribute])*
        #[repr(transparent)]
        #[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name([u8; $length]);

        impl $name {
            /// Exact wire width.
            pub const LENGTH: usize = $length;

            /// Creates a value from its exact wire representation.
            #[must_use]
            pub const fn from_bytes(bytes: [u8; $length]) -> Self {
                Self(bytes)
            }

            /// Borrows the exact wire representation.
            #[must_use]
            pub const fn as_bytes(&self) -> &[u8; $length] {
                &self.0
            }

            /// Returns the exact wire representation.
            #[must_use]
            pub const fn into_bytes(self) -> [u8; $length] {
                self.0
            }
        }

        impl TryFrom<&[u8]> for $name {
            type Error = FixedBytesLengthError;

            fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
                let array = bytes.try_into().map_err(|_| FixedBytesLengthError {
                    kind: stringify!($name),
                    expected: $length,
                    actual: bytes.len(),
                })?;
                Ok(Self::from_bytes(array))
            }
        }

        impl From<[u8; $length]> for $name {
            fn from(bytes: [u8; $length]) -> Self {
                Self::from_bytes(bytes)
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(concat!(stringify!($name), "(<opaque>)"))
            }
        }
    };
}

fixed_bytes!(
    /// Random installation identifier; identity proof remains key based.
    InstallationId,
    16
);
fixed_bytes!(
    /// One authenticated secure-session identifier.
    SessionId,
    16
);
fixed_bytes!(
    /// Logical request identifier preserved across a carrier retry.
    RequestId,
    16
);
fixed_bytes!(
    /// Public profile release identifier.
    ProfileId,
    16
);
fixed_bytes!(
    /// Local account runtime slot identifier.
    AccountSlotId,
    16
);
fixed_bytes!(
    /// Opaque exchange correlation identifier.
    ExchangeId,
    16
);
fixed_bytes!(
    /// Generic incident correlation identifier.
    IncidentId,
    16
);
fixed_bytes!(
    /// Client-created identifier for one bounded action flow.
    ActionFlowId,
    16
);
fixed_bytes!(
    /// Server-created identifier for one transport action.
    ActionId,
    16
);
fixed_bytes!(
    /// Random identifier for one idempotent daily telemetry report.
    TelemetryReportId,
    16
);
fixed_bytes!(
    /// Thirty-two-byte digest value.
    Digest32,
    32
);
