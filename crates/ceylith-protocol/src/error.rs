use core::fmt;

/// Outer frame class used in redacted codec errors.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FrameKind {
    /// Handshake envelope.
    Handshake,
    /// Post-handshake encrypted frame.
    Secure,
    /// Encrypted inner protobuf.
    Inner,
}

/// Length class used in redacted codec errors.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LengthKind {
    /// Complete outer frame.
    OuterFrame,
    /// Handshake payload.
    HandshakePayload,
    /// Encrypted frame ciphertext.
    Ciphertext,
    /// Decoded inner protobuf.
    InnerFrame,
    /// A bounded public-contract field.
    Field,
    /// A bounded repeated field.
    Collection,
}

/// Closed codec failure that never includes payload bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CodecError {
    /// Input ended before the declared frame boundary.
    Truncated {
        /// Required byte length.
        needed: usize,
        /// Available byte length.
        available: usize,
    },
    /// Input contained bytes after one declared frame.
    TrailingBytes {
        /// Declared byte length.
        expected: usize,
        /// Actual byte length.
        actual: usize,
    },
    /// Fixed frame magic did not match.
    InvalidMagic {
        /// Frame being decoded.
        frame: FrameKind,
    },
    /// Version is not compiled into this release.
    UnsupportedVersion,
    /// Handshake step is invalid.
    InvalidHandshakeStep,
    /// Reserved flags were set.
    InvalidFlags,
    /// A declared length exceeded its bound.
    LengthLimitExceeded {
        /// Length class.
        kind: LengthKind,
        /// Maximum accepted length.
        limit: usize,
        /// Rejected length.
        actual: usize,
    },
    /// Checked length arithmetic overflowed.
    LengthOverflow,
    /// Protobuf encoding or decoding failed.
    Protobuf,
    /// Inner contract version is missing or unsupported.
    InvalidContract,
    /// Inner body is absent or not compiled into this release.
    UnsupportedBody,
    /// A public field has an invalid fixed width or value.
    InvalidField,
}

impl fmt::Display for CodecError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Ceylith public frame rejected")
    }
}

impl std::error::Error for CodecError {}

/// Structural profile-decision failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProfileError {
    /// Status is missing or unknown.
    InvalidStatus,
    /// Profile identifier has an invalid width.
    InvalidProfileId,
    /// Ready outcome omitted a required value.
    IncompleteReady,
    /// Non-ready outcome carried a manifest.
    UnexpectedManifest,
    /// A manifest or signature exceeded its public bound.
    Bounds,
}

impl fmt::Display for ProfileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("profile decision rejected")
    }
}

impl std::error::Error for ProfileError {}

/// Invalid bounded opaque-slot collection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OpaqueError {
    /// An opaque slot identifier was zero.
    InvalidSlotId,
    /// An opaque slot value was empty or too large.
    InvalidSlotValue,
    /// Opaque slot identifiers were duplicated.
    DuplicateSlot,
    /// The slot count or aggregate length exceeded its public bound.
    Bounds,
}

impl fmt::Display for OpaqueError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("opaque slot collection rejected")
    }
}

impl std::error::Error for OpaqueError {}
