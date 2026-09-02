//! QQ message model boundary for Lirvena.

mod decoder;
mod error;
mod model;
mod outbound;
mod proto;
mod rich_text;

pub use decoder::{MessageDecoder, MessageDisposition};
pub use error::MessageDecodeError;
pub use model::{MessageClass, MessageEnvelope, MessagePayload, MessageRoute};
pub use outbound::{
    SendTextInput, SendTextOutcome, SendTextTarget, encode_text_message,
    parse_send_message_response,
};
pub use rich_text::{
    FaceKind, FaceSegment, ImageSegment, MediaFile, MediaScope, MentionSegment, MentionTarget,
    OpaqueAttachment, RichTextElement, RichTextMessage, Segment, VideoSegment, VoiceSegment,
    decode_rich_text,
};
