//! QQ message model boundary for Lirvena.

mod decoder;
mod error;
mod model;
mod notice;
mod outbound;
mod proto;
mod request;
mod rich_text;

pub use decoder::{MessageDecoder, MessageDisposition};
pub use error::MessageDecodeError;
pub use model::{MessageClass, MessageEnvelope, MessagePayload, MessageRoute};
pub use notice::{GroupNotice, MemberDecreaseKind, MemberIncreaseKind, decode_group_notice};
pub use outbound::{
    OutboundSegment, SendMessageInput, SendTextInput, SendTextOutcome, SendTextTarget,
    encode_message, encode_text_message, parse_send_message_response,
};
pub use request::{GroupRequestSignal, decode_group_request_signal};
pub use rich_text::{
    FaceKind, FaceSegment, ImageSegment, MediaFile, MediaScope, MentionSegment, MentionTarget,
    OpaqueAttachment, RichTextElement, RichTextMessage, Segment, VideoSegment, VoiceSegment,
    decode_rich_text,
};
