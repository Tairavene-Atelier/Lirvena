use prost::Message;

#[derive(Clone, PartialEq, Message)]
pub(super) struct Push {
    #[prost(message, optional, tag = "1")]
    pub message: Option<PushBody>,
    #[prost(int32, optional, tag = "3")]
    pub status: Option<i32>,
    #[prost(bytes = "vec", optional, tag = "4")]
    pub system_event: Option<Vec<u8>>,
    #[prost(int32, optional, tag = "5")]
    pub ping_flag: Option<i32>,
    #[prost(int32, optional, tag = "9")]
    pub general_flag: Option<i32>,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct PushBody {
    #[prost(message, optional, tag = "1")]
    pub response: Option<ResponseHead>,
    #[prost(message, optional, tag = "2")]
    pub content: Option<ContentHead>,
    #[prost(message, optional, tag = "3")]
    pub body: Option<MessageBody>,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct ResponseHead {
    #[prost(uint32, tag = "1")]
    pub from_uin: u32,
    #[prost(string, optional, tag = "2")]
    pub from_uid: Option<String>,
    #[prost(uint32, tag = "3")]
    pub message_type: u32,
    #[prost(uint32, tag = "4")]
    pub signature_map: u32,
    #[prost(uint32, tag = "5")]
    pub to_uin: u32,
    #[prost(string, optional, tag = "6")]
    pub to_uid: Option<String>,
    #[prost(message, optional, tag = "7")]
    pub forward: Option<ResponseForward>,
    #[prost(message, optional, tag = "8")]
    pub group: Option<ResponseGroup>,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct ResponseForward {
    #[prost(string, optional, tag = "6")]
    pub friend_name: Option<String>,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct ResponseGroup {
    #[prost(uint32, tag = "1")]
    pub group_uin: u32,
    #[prost(string, tag = "4")]
    pub member_name: String,
    #[prost(uint32, tag = "5")]
    pub field_five: u32,
    #[prost(string, tag = "7")]
    pub group_name: String,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct ContentHead {
    #[prost(uint32, tag = "1")]
    pub message_type: u32,
    #[prost(uint32, optional, tag = "2")]
    pub sub_type: Option<u32>,
    #[prost(uint32, optional, tag = "3")]
    pub direct_command: Option<u32>,
    #[prost(int64, optional, tag = "4")]
    pub random: Option<i64>,
    #[prost(uint64, optional, tag = "5")]
    pub sequence: Option<u64>,
    #[prost(int64, optional, tag = "6")]
    pub timestamp: Option<i64>,
    #[prost(int64, optional, tag = "7")]
    pub package_count: Option<i64>,
    #[prost(uint32, optional, tag = "8")]
    pub package_index: Option<u32>,
    #[prost(uint32, optional, tag = "9")]
    pub division_sequence: Option<u32>,
    #[prost(uint32, tag = "10")]
    pub auto_reply: u32,
    #[prost(uint32, optional, tag = "11")]
    pub direct_message_sequence: Option<u32>,
    #[prost(uint64, optional, tag = "12")]
    pub message_uid: Option<u64>,
    #[prost(message, optional, tag = "15")]
    pub forward: Option<ForwardHead>,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct ForwardHead {
    #[prost(uint32, optional, tag = "1")]
    pub field_one: Option<u32>,
    #[prost(uint32, optional, tag = "2")]
    pub field_two: Option<u32>,
    #[prost(uint32, optional, tag = "3")]
    pub field_three: Option<u32>,
    #[prost(string, optional, tag = "4")]
    pub encoded_value: Option<String>,
    #[prost(string, optional, tag = "5")]
    pub avatar: Option<String>,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct MessageBody {
    #[prost(bytes = "vec", optional, tag = "1")]
    pub rich_text: Option<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "2")]
    pub content: Option<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "3")]
    pub encrypted_content: Option<Vec<u8>>,
}
