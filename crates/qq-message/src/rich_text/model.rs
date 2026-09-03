/// Bounded rich-text body projected from one authenticated message.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RichTextMessage {
    elements: Box<[RichTextElement]>,
    attributes: Option<OpaqueAttachment>,
    file: Option<OpaqueAttachment>,
    voice: Option<OpaqueAttachment>,
}

impl RichTextMessage {
    pub(super) fn new(
        elements: Vec<RichTextElement>,
        attributes: Option<Vec<u8>>,
        file: Option<Vec<u8>>,
        voice: Option<Vec<u8>>,
    ) -> Self {
        Self {
            elements: elements.into_boxed_slice(),
            attributes: attributes.map(OpaqueAttachment::new),
            file: file.map(OpaqueAttachment::new),
            voice: voice.map(OpaqueAttachment::new),
        }
    }

    /// Returns message elements in their original wire order.
    #[must_use]
    pub const fn elements(&self) -> &[RichTextElement] {
        &self.elements
    }

    /// Returns opaque rich-text attributes for a later compiled decoder.
    #[must_use]
    pub const fn attributes(&self) -> Option<&OpaqueAttachment> {
        self.attributes.as_ref()
    }

    /// Returns an opaque attached-file descriptor for a later compiled decoder.
    #[must_use]
    pub const fn file(&self) -> Option<&OpaqueAttachment> {
        self.file.as_ref()
    }

    /// Returns an opaque voice descriptor for a later compiled decoder.
    #[must_use]
    pub const fn voice(&self) -> Option<&OpaqueAttachment> {
        self.voice.as_ref()
    }
}

/// One original element and its conservative compiled projection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RichTextElement {
    segment: Segment,
    encoded: Box<[u8]>,
}

impl RichTextElement {
    pub(super) fn new(segment: Segment, encoded: Vec<u8>) -> Self {
        Self {
            segment,
            encoded: encoded.into_boxed_slice(),
        }
    }

    /// Returns the compiled semantic segment, or `Unsupported` when evidence is insufficient.
    #[must_use]
    pub const fn segment(&self) -> &Segment {
        &self.segment
    }

    /// Returns the complete encoded element for future compiled decoders.
    #[must_use]
    pub const fn encoded(&self) -> &[u8] {
        &self.encoded
    }
}

/// Closed initial set of evidence-backed message segments.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Segment {
    /// Ordinary text.
    Text(String),
    /// A user or everyone mention.
    Mention(MentionSegment),
    /// A standard or animated face.
    Face(FaceSegment),
    /// Incoming image metadata.
    Image(super::ImageSegment),
    /// Incoming video metadata.
    Video(super::VideoSegment),
    /// Incoming voice metadata.
    Voice(super::VoiceSegment),
    /// Incoming JSON rich content.
    Json(String),
    /// Incoming XML rich content.
    Xml(XmlSegment),
    /// Incoming shake/poke content.
    Poke(PokeSegment),
    /// A valid element without one unambiguous compiled projection yet.
    Unsupported,
}

/// Incoming XML rich content and its QQ service identifier.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XmlSegment {
    body: String,
    service_id: i32,
}

impl XmlSegment {
    pub(super) const fn new(body: String, service_id: i32) -> Self {
        Self { body, service_id }
    }

    /// Returns the decoded XML body.
    #[must_use]
    pub fn body(&self) -> &str {
        &self.body
    }

    /// Returns the QQ rich-message service identifier.
    #[must_use]
    pub const fn service_id(&self) -> i32 {
        self.service_id
    }
}

/// Incoming QQ shake/poke content.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PokeSegment {
    kind: u32,
    strength: u32,
}

impl PokeSegment {
    pub(super) const fn new(kind: u32, strength: u32) -> Self {
        Self { kind, strength }
    }

    /// Returns the poke kind.
    #[must_use]
    pub const fn kind(self) -> u32 {
        self.kind
    }

    /// Returns the poke strength.
    #[must_use]
    pub const fn strength(self) -> u32 {
        self.strength
    }
}

/// Bounded mention display and target.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MentionSegment {
    display: String,
    target: MentionTarget,
}

impl MentionSegment {
    pub(super) const fn new(display: String, target: MentionTarget) -> Self {
        Self { display, target }
    }

    /// Returns the sender-provided display text.
    #[must_use]
    pub fn display(&self) -> &str {
        &self.display
    }

    /// Returns the resolved target form carried by the message.
    #[must_use]
    pub const fn target(&self) -> &MentionTarget {
        &self.target
    }
}

/// Mention target form available without a network lookup.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MentionTarget {
    /// All members in the current group.
    Everyone,
    /// Numeric account identifier.
    Account(u32),
    /// Current-generation string user identifier.
    User(String),
    /// Mention metadata was valid but did not carry a resolvable target.
    Unresolved,
}

/// Face segment projected from a legacy or common element.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FaceSegment {
    id: u32,
    kind: FaceKind,
}

impl FaceSegment {
    pub(super) const fn new(id: u32, kind: FaceKind) -> Self {
        Self { id, kind }
    }

    /// Returns the QQ face identifier.
    #[must_use]
    pub const fn id(self) -> u32 {
        self.id
    }

    /// Returns the face presentation class.
    #[must_use]
    pub const fn kind(self) -> FaceKind {
        self.kind
    }
}

/// Face presentation class.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FaceKind {
    /// Ordinary face presentation.
    Standard,
    /// Animated large-face presentation.
    Animated,
}

/// Bounded encoded attachment awaiting a dedicated compiled decoder.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpaqueAttachment(Box<[u8]>);

impl OpaqueAttachment {
    fn new(encoded: Vec<u8>) -> Self {
        Self(encoded.into_boxed_slice())
    }

    /// Returns the encoded attachment.
    #[must_use]
    pub const fn encoded(&self) -> &[u8] {
        &self.0
    }
}
