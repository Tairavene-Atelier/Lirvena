//! QQ message model boundary for Lirvena.

mod decoder;
mod error;
mod friend_request;
mod model;
mod notice;
mod outbound;
mod proto;
mod recall;
mod recall_response;
mod request;
mod rich_text;

pub use decoder::{MessageDecoder, MessageDisposition};
pub use error::MessageDecodeError;
pub use friend_request::{FriendRequestSignal, decode_friend_request_signal};
pub use model::{MessageClass, MessageEnvelope, MessagePayload, MessageRoute};
pub use notice::{GroupNotice, MemberDecreaseKind, MemberIncreaseKind, decode_group_notice};
pub use outbound::{
    OutboundSegment, SendMessageInput, SendTextInput, SendTextOutcome, SendTextTarget,
    encode_message, encode_text_message, parse_send_message_response,
};
pub use recall::{
    GroupRecallInput, PrivateRecallInput, encode_group_recall, encode_private_recall,
};
pub use recall_response::{validate_group_recall_response, validate_private_recall_response};
pub use request::{GroupRequestSignal, decode_group_request_signal};
pub use rich_text::{
    FaceKind, FaceSegment, ImageSegment, MediaFile, MediaScope, MentionSegment, MentionTarget,
    OpaqueAttachment, RichTextElement, RichTextMessage, Segment, VideoSegment, VoiceSegment,
    decode_rich_text,
};
