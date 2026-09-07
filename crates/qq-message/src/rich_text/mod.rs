mod decode;
pub(crate) mod location;
mod media;
mod media_decode;
mod media_legacy;
mod model;
mod proto;

pub use decode::decode_rich_text;
pub use location::LocationSegment;
pub use media::{ImageSegment, MediaFile, MediaScope, VideoSegment, VoiceSegment};
pub use model::{
    FaceKind, FaceSegment, ForwardSegment, MentionSegment, MentionTarget, OpaqueAttachment,
    PokeSegment, ReplySegment, RichTextElement, RichTextMessage, Segment, XmlSegment,
};
