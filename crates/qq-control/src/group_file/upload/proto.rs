use prost::Message;

#[derive(Clone, PartialEq, Message)]
pub(super) struct UploadEnvelope {
    #[prost(message, optional, tag = "1")]
    pub(super) upload: Option<UploadRequest>,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct UploadRequest {
    #[prost(uint32, tag = "1")]
    pub(super) group_uin: u32,
    #[prost(uint32, tag = "2")]
    pub(super) app_id: u32,
    #[prost(uint32, tag = "3")]
    pub(super) business_id: u32,
    #[prost(uint32, tag = "4")]
    pub(super) entrance: u32,
    #[prost(string, tag = "5")]
    pub(super) target_directory: String,
    #[prost(string, tag = "6")]
    pub(super) file_name: String,
    #[prost(string, tag = "7")]
    pub(super) local_path: String,
    #[prost(uint64, tag = "8")]
    pub(super) file_size: u64,
    #[prost(bytes = "vec", tag = "9")]
    pub(super) sha1: Vec<u8>,
    #[prost(bytes = "vec", tag = "10")]
    pub(super) sha3: Vec<u8>,
    #[prost(bytes = "vec", tag = "11")]
    pub(super) md5: Vec<u8>,
    #[prost(bool, tag = "15")]
    pub(super) field15: bool,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct UploadResponseEnvelope {
    #[prost(message, optional, tag = "1")]
    pub(super) upload: Option<UploadResponse>,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct UploadResponse {
    #[prost(int32, tag = "1")]
    pub(super) result: i32,
    #[prost(string, tag = "4")]
    pub(super) upload_host: String,
    #[prost(string, tag = "7")]
    pub(super) file_id: String,
    #[prost(bytes = "vec", tag = "8")]
    pub(super) check_key: Vec<u8>,
    #[prost(bytes = "vec", tag = "9")]
    pub(super) upload_key: Vec<u8>,
    #[prost(bool, tag = "10")]
    pub(super) file_exists: bool,
    #[prost(uint32, tag = "14")]
    pub(super) upload_port: u32,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct CompleteEnvelope {
    #[prost(message, optional, tag = "5")]
    pub(super) body: Option<CompleteBody>,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct CompleteBody {
    #[prost(uint32, tag = "1")]
    pub(super) group_uin: u32,
    #[prost(uint32, tag = "2")]
    pub(super) kind: u32,
    #[prost(message, optional, tag = "3")]
    pub(super) info: Option<CompleteInfo>,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct CompleteInfo {
    #[prost(uint32, tag = "1")]
    pub(super) business_type: u32,
    #[prost(string, tag = "2")]
    pub(super) file_id: String,
    #[prost(uint32, tag = "3")]
    pub(super) random: u32,
    #[prost(bool, tag = "5")]
    pub(super) field5: bool,
}
