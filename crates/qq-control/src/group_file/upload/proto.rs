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

#[derive(Clone, PartialEq, Message)]
pub(super) struct HighwayExtension {
    #[prost(int32, tag = "1")]
    pub(super) field1: i32,
    #[prost(int32, tag = "2")]
    pub(super) field2: i32,
    #[prost(message, optional, tag = "100")]
    pub(super) entry: Option<HighwayEntry>,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct HighwayEntry {
    #[prost(message, optional, tag = "100")]
    pub(super) business: Option<HighwayBusiness>,
    #[prost(message, optional, tag = "200")]
    pub(super) file: Option<HighwayFile>,
    #[prost(message, optional, tag = "300")]
    pub(super) client: Option<HighwayClientInfo>,
    #[prost(message, optional, tag = "400")]
    pub(super) name: Option<HighwayFileName>,
    #[prost(message, optional, tag = "500")]
    pub(super) host: Option<HighwayHostConfig>,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct HighwayBusiness {
    #[prost(uint64, tag = "100")]
    pub(super) sender: u64,
    #[prost(uint64, tag = "200")]
    pub(super) receiver: u64,
    #[prost(uint64, tag = "400")]
    pub(super) group: u64,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct HighwayFile {
    #[prost(uint64, tag = "100")]
    pub(super) file_size: u64,
    #[prost(bytes = "vec", tag = "200")]
    pub(super) md5: Vec<u8>,
    #[prost(bytes = "vec", tag = "300")]
    pub(super) check_key: Vec<u8>,
    #[prost(bytes = "vec", tag = "400")]
    pub(super) second_md5: Vec<u8>,
    #[prost(string, tag = "600")]
    pub(super) file_id: String,
    #[prost(bytes = "vec", tag = "700")]
    pub(super) upload_key: Vec<u8>,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct HighwayClientInfo {
    #[prost(int32, tag = "100")]
    pub(super) client_type: i32,
    #[prost(string, tag = "200")]
    pub(super) app_id: String,
    #[prost(int32, tag = "300")]
    pub(super) terminal_type: i32,
    #[prost(string, tag = "400")]
    pub(super) client_version: String,
    #[prost(int32, tag = "600")]
    pub(super) field6: i32,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct HighwayFileName {
    #[prost(string, tag = "100")]
    pub(super) file_name: String,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct HighwayHostConfig {
    #[prost(message, repeated, tag = "200")]
    pub(super) hosts: Vec<HighwayHost>,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct HighwayHost {
    #[prost(message, optional, tag = "1")]
    pub(super) url: Option<HighwayUrl>,
    #[prost(uint32, tag = "2")]
    pub(super) port: u32,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct HighwayUrl {
    #[prost(int32, tag = "1")]
    pub(super) field1: i32,
    #[prost(string, tag = "2")]
    pub(super) host: String,
}
