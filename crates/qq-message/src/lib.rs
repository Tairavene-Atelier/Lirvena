//! QQ message model boundary for Lirvena.

mod decoder;
mod error;
mod event_payload;
mod friend_recall;
mod friend_request;
mod group_mute;
mod group_name;
mod group_recall;
mod history;
mod long_message;
mod market_face;
mod model;
mod notice;
mod outbound;
mod proto;
mod reaction;
mod read_report;
mod recall;
mod recall_response;
mod request;
mod rich_content;
mod rich_text;

pub use decoder::{MessageDecoder, MessageDisposition};
pub use error::MessageDecodeError;
pub use friend_recall::{FriendRecall, decode_friend_recall};
pub use friend_request::{FriendRequestSignal, decode_friend_request_signal};
pub use group_mute::{GroupMute, decode_group_mute};
pub use group_name::{GroupNameChange, decode_group_name_change};
pub use group_recall::{GroupRecall, decode_group_recalls};
pub use history::{
    FRIEND_HISTORY_ROUTE, GROUP_HISTORY_ROUTE, HistoricalMessage, decode_friend_history_response,
    decode_group_history_response, encode_friend_history_request, encode_group_history_request,
};
pub use long_message::{
    LongMessageTarget, encode_long_message_receive, encode_long_message_send,
    parse_long_message_receive, parse_long_message_send,
};
pub use market_face::MarketFaceSegment;
pub use model::{MessageClass, MessageEnvelope, MessagePayload, MessageRoute};
pub use notice::{GroupNotice, MemberDecreaseKind, MemberIncreaseKind, decode_group_notice};
pub use outbound::{
    ForwardEntryInput, OutboundSegment, PrivateFileMessageInput, SendMessageInput, SendTextInput,
    SendTextOutcome, SendTextTarget, encode_forward_entry, encode_message,
    encode_private_file_message, encode_text_message, parse_send_message_response,
    validate_private_file_message_response,
};
pub use reaction::{GroupReaction, decode_group_reaction};
pub use read_report::{ReadReportInput, encode_read_report, validate_read_report_response};
pub use recall::{
    GroupRecallInput, PrivateRecallInput, encode_group_recall, encode_private_recall,
};
pub use recall_response::{validate_group_recall_response, validate_private_recall_response};
pub use request::{GroupRequestSignal, decode_group_request_signal};
pub use rich_text::{
    FaceKind, FaceSegment, ForwardSegment, ImageSegment, LocationSegment, MediaFile, MediaScope,
    MentionSegment, MentionTarget, OpaqueAttachment, PokeSegment, ReplySegment, RichTextElement,
    RichTextMessage, Segment, VideoSegment, VoiceSegment, XmlSegment, decode_rich_text,
};
