use prost::Message;

#[derive(Clone, PartialEq, Message)]
pub(super) struct RichRequest {
    #[prost(message, optional, tag = "1")]
    pub head: Option<RequestHead>,
    #[prost(message, optional, tag = "2")]
    pub upload: Option<UploadRequest>,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct RequestHead {
    #[prost(message, optional, tag = "1")]
    pub common: Option<CommonHead>,
    #[prost(message, optional, tag = "2")]
    pub scene: Option<Scene>,
    #[prost(message, optional, tag = "3")]
    pub client: Option<ClientMeta>,
}

#[derive(Clone, Copy, PartialEq, Message)]
pub(super) struct CommonHead {
    #[prost(uint32, tag = "1")]
    pub request_id: u32,
    #[prost(uint32, tag = "2")]
    pub command: u32,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct Scene {
    #[prost(uint32, tag = "101")]
    pub request_type: u32,
    #[prost(uint32, tag = "102")]
    pub business_type: u32,
    #[prost(uint32, tag = "200")]
    pub kind: u32,
    #[prost(message, optional, tag = "201")]
    pub direct: Option<DirectTarget>,
    #[prost(message, optional, tag = "202")]
    pub group: Option<GroupTarget>,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct DirectTarget {
    #[prost(uint32, tag = "1")]
    pub account_type: u32,
    #[prost(string, tag = "2")]
    pub uid: String,
}

#[derive(Clone, Copy, PartialEq, Message)]
pub(super) struct GroupTarget {
    #[prost(uint32, tag = "1")]
    pub group_code: u32,
}

#[derive(Clone, Copy, PartialEq, Message)]
pub(super) struct ClientMeta {
    #[prost(uint32, tag = "1")]
    pub agent_type: u32,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct UploadRequest {
    #[prost(message, repeated, tag = "1")]
    pub files: Vec<UploadInfo>,
    #[prost(bool, tag = "2")]
    pub try_fast_upload: bool,
    #[prost(bool, tag = "3")]
    pub server_sends_message: bool,
    #[prost(uint64, tag = "4")]
    pub client_random_id: u64,
    #[prost(uint32, tag = "5")]
    pub compatibility_scene: u32,
    #[prost(message, optional, tag = "6")]
    pub business: Option<BusinessInfo>,
    #[prost(uint32, tag = "7")]
    pub client_sequence: u32,
    #[prost(bool, tag = "8")]
    pub no_compatibility_message: bool,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct UploadInfo {
    #[prost(message, optional, tag = "1")]
    pub file: Option<FileInfo>,
    #[prost(uint32, tag = "2")]
    pub sub_file_type: u32,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct FileInfo {
    #[prost(uint32, tag = "1")]
    pub size: u32,
    #[prost(string, tag = "2")]
    pub md5: String,
    #[prost(string, tag = "3")]
    pub sha1: String,
    #[prost(string, tag = "4")]
    pub name: String,
    #[prost(message, optional, tag = "5")]
    pub kind: Option<FileType>,
    #[prost(uint32, tag = "6")]
    pub width: u32,
    #[prost(uint32, tag = "7")]
    pub height: u32,
    #[prost(uint32, tag = "8")]
    pub duration: u32,
    #[prost(uint32, tag = "9")]
    pub original: u32,
}

#[derive(Clone, Copy, PartialEq, Message)]
pub(super) struct FileType {
    #[prost(uint32, tag = "1")]
    pub kind: u32,
    #[prost(uint32, tag = "2")]
    pub picture_format: u32,
    #[prost(uint32, tag = "3")]
    pub video_format: u32,
    #[prost(uint32, tag = "4")]
    pub voice_format: u32,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct BusinessInfo {
    #[prost(message, optional, tag = "1")]
    pub picture: Option<PictureBusiness>,
    #[prost(message, optional, tag = "2")]
    pub video: Option<VideoBusiness>,
    #[prost(message, optional, tag = "3")]
    pub voice: Option<VoiceBusiness>,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct PictureBusiness {
    #[prost(uint32, tag = "1")]
    pub business_type: u32,
    #[prost(string, tag = "2")]
    pub summary: String,
    #[prost(bytes = "vec", tag = "11")]
    pub reserve: Vec<u8>,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct VideoBusiness {
    #[prost(bytes = "vec", tag = "3")]
    pub reserve: Vec<u8>,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct VoiceBusiness {
    #[prost(bytes = "vec", tag = "11")]
    pub reserve: Vec<u8>,
    #[prost(bytes = "vec", tag = "12")]
    pub protobuf_reserve: Vec<u8>,
    #[prost(bytes = "vec", tag = "13")]
    pub general_flags: Vec<u8>,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct RichResponse {
    #[prost(message, optional, tag = "1")]
    pub head: Option<ResponseHead>,
    #[prost(message, optional, tag = "2")]
    pub upload: Option<UploadResponse>,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct ResponseHead {
    #[prost(uint32, tag = "2")]
    pub return_code: u32,
    #[prost(string, tag = "3")]
    pub message: String,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct UploadResponse {
    #[prost(string, tag = "1")]
    pub upload_key: String,
    #[prost(message, repeated, tag = "3")]
    pub ipv4: Vec<Ipv4>,
    #[prost(bytes = "vec", tag = "6")]
    pub message_info: Vec<u8>,
    #[prost(bytes = "vec", tag = "8")]
    pub compatibility_message: Vec<u8>,
    #[prost(message, repeated, tag = "10")]
    pub sub_files: Vec<SubFileInfo>,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct SubFileInfo {
    #[prost(uint32, tag = "1")]
    pub sub_file_type: u32,
    #[prost(string, tag = "2")]
    pub upload_key: String,
    #[prost(message, repeated, tag = "4")]
    pub ipv4: Vec<Ipv4>,
}

#[derive(Clone, Copy, PartialEq, Message)]
pub(super) struct Ipv4 {
    #[prost(uint32, tag = "1")]
    pub external_address: u32,
    #[prost(uint32, tag = "2")]
    pub external_port: u32,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct RawMessageInfo {
    #[prost(bytes = "vec", repeated, tag = "1")]
    pub bodies: Vec<Vec<u8>>,
    #[prost(bytes = "vec", tag = "2")]
    pub business: Vec<u8>,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct RawMessageBody {
    #[prost(bytes = "vec", tag = "1")]
    pub index: Vec<u8>,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct IndexNode {
    #[prost(message, optional, tag = "1")]
    pub info: Option<FileInfo>,
    #[prost(string, tag = "2")]
    pub uuid: String,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct HighwayExtension {
    #[prost(string, tag = "1")]
    pub uuid: String,
    #[prost(string, tag = "2")]
    pub upload_key: String,
    #[prost(message, optional, tag = "5")]
    pub network: Option<HighwayNetwork>,
    #[prost(bytes = "vec", repeated, tag = "6")]
    pub message_bodies: Vec<Vec<u8>>,
    #[prost(uint32, tag = "10")]
    pub block_size: u32,
    #[prost(message, optional, tag = "11")]
    pub hashes: Option<HighwayHashes>,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct HighwayHashes {
    #[prost(bytes = "vec", repeated, tag = "1")]
    pub sha1: Vec<Vec<u8>>,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct HighwayNetwork {
    #[prost(message, repeated, tag = "1")]
    pub addresses: Vec<HighwayAddress>,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct HighwayAddress {
    #[prost(message, optional, tag = "1")]
    pub domain: Option<HighwayDomain>,
    #[prost(uint32, tag = "2")]
    pub port: u32,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct HighwayDomain {
    #[prost(bool, tag = "1")]
    pub enabled: bool,
    #[prost(string, tag = "2")]
    pub address: String,
}
