//! QQ message model boundary for Lirvena.

mod decoder;
mod error;
mod model;
mod proto;
mod rich_text;

pub use decoder::{MessageDecoder, MessageDisposition};
pub use error::MessageDecodeError;
pub use model::{MessageClass, MessageEnvelope, MessagePayload, MessageRoute};
pub use rich_text::{
    FaceKind, FaceSegment, ImageSegment, MediaFile, MediaScope, MentionSegment, MentionTarget,
    OpaqueAttachment, RichTextElement, RichTextMessage, Segment, VideoSegment, VoiceSegment,
    decode_rich_text,
};
