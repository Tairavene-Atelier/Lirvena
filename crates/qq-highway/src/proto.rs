use prost::Message;

#[derive(Clone, PartialEq, Message)]
pub(super) struct SessionOuter {
    #[prost(message, optional, tag = "1281")]
    pub connection: Option<SessionRequestWire>,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct SessionRequestWire {
    #[prost(int32, tag = "1")]
    pub field_1: i32,
    #[prost(int32, tag = "2")]
    pub field_2: i32,
    #[prost(int32, tag = "3")]
    pub field_3: i32,
    #[prost(int32, tag = "4")]
    pub field_4: i32,
    #[prost(string, tag = "5")]
    pub tgt_hex: String,
    #[prost(int32, tag = "6")]
    pub field_6: i32,
    #[prost(int32, repeated, tag = "7")]
    pub service_types: Vec<i32>,
    #[prost(int32, tag = "9")]
    pub field_9: i32,
    #[prost(int32, tag = "10")]
    pub field_10: i32,
    #[prost(int32, tag = "11")]
    pub field_11: i32,
    #[prost(string, tag = "15")]
    pub version: String,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct SessionResponseOuter {
    #[prost(message, optional, tag = "1281")]
    pub connection: Option<SessionResponseWire>,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct SessionResponseWire {
    #[prost(bytes = "vec", tag = "1")]
    pub ticket: Vec<u8>,
    #[prost(bytes = "vec", tag = "2")]
    pub session_key: Vec<u8>,
    #[prost(message, repeated, tag = "3")]
    pub servers: Vec<ServerInfoWire>,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct ServerInfoWire {
    #[prost(uint32, tag = "1")]
    pub service_type: u32,
    #[prost(message, repeated, tag = "2")]
    pub addresses: Vec<ServerAddressWire>,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct ServerAddressWire {
    #[prost(uint32, tag = "1")]
    pub address_type: u32,
    #[prost(uint32, tag = "2")]
    pub ipv4: u32,
    #[prost(uint32, tag = "3")]
    pub port: u32,
    #[prost(uint32, tag = "4")]
    pub area: u32,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct RequestHeadWire {
    #[prost(message, optional, tag = "1")]
    pub base: Option<BaseHeadWire>,
    #[prost(message, optional, tag = "2")]
    pub segment: Option<SegmentHeadWire>,
    #[prost(bytes = "vec", tag = "3")]
    pub extension: Vec<u8>,
    #[prost(uint64, tag = "4")]
    pub timestamp: u64,
    #[prost(message, optional, tag = "5")]
    pub login: Option<LoginHeadWire>,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct ResponseHeadWire {
    #[prost(message, optional, tag = "1")]
    pub base: Option<BaseHeadWire>,
    #[prost(message, optional, tag = "2")]
    pub segment: Option<SegmentHeadWire>,
    #[prost(uint32, tag = "3")]
    pub error_code: u32,
    #[prost(uint32, tag = "4")]
    pub allow_retry: u32,
    #[prost(bytes = "vec", tag = "7")]
    pub extension: Vec<u8>,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct BaseHeadWire {
    #[prost(uint32, tag = "1")]
    pub version: u32,
    #[prost(string, tag = "2")]
    pub uin: String,
    #[prost(string, tag = "3")]
    pub command: String,
    #[prost(uint32, tag = "4")]
    pub sequence: u32,
    #[prost(uint32, tag = "5")]
    pub retry_times: u32,
    #[prost(uint32, tag = "6")]
    pub app_id: u32,
    #[prost(uint32, tag = "7")]
    pub data_flag: u32,
    #[prost(uint32, tag = "8")]
    pub command_id: u32,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct SegmentHeadWire {
    #[prost(uint32, tag = "1")]
    pub service_id: u32,
    #[prost(uint64, tag = "2")]
    pub file_size: u64,
    #[prost(uint64, tag = "3")]
    pub offset: u64,
    #[prost(uint32, tag = "4")]
    pub data_length: u32,
    #[prost(uint32, tag = "5")]
    pub return_code: u32,
    #[prost(bytes = "vec", tag = "6")]
    pub ticket: Vec<u8>,
    #[prost(bytes = "vec", tag = "8")]
    pub block_md5: Vec<u8>,
    #[prost(bytes = "vec", tag = "9")]
    pub file_md5: Vec<u8>,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct LoginHeadWire {
    #[prost(uint32, tag = "1")]
    pub signature_type: u32,
    #[prost(bytes = "vec", tag = "2")]
    pub signature: Vec<u8>,
    #[prost(uint32, tag = "3")]
    pub app_id: u32,
}
