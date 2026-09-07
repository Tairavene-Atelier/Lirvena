use prost::Message;

pub(crate) struct HighwayFileExtensionSpec<'a> {
    pub(crate) sender_uin: u64,
    pub(crate) receiver_uin: u64,
    pub(crate) group_uin: u64,
    pub(crate) file_size: u64,
    pub(crate) md5: &'a [u8; 16],
    pub(crate) check_key: &'a [u8],
    pub(crate) file_id: &'a str,
    pub(crate) upload_key: &'a [u8],
    pub(crate) file_name: &'a str,
    pub(crate) upload_host: &'a str,
    pub(crate) upload_port: u32,
    pub(crate) private_trailer: bool,
}

pub(crate) fn encode_highway_file_extension(spec: &HighwayFileExtensionSpec<'_>) -> Vec<u8> {
    HighwayExtension {
        field1: 100,
        field2: 1,
        entry: Some(HighwayEntry {
            business: Some(HighwayBusiness {
                sender: spec.sender_uin,
                receiver: spec.receiver_uin,
                group: spec.group_uin,
            }),
            file: Some(HighwayFile {
                file_size: spec.file_size,
                md5: spec.md5.to_vec(),
                check_key: spec.check_key.to_vec(),
                second_md5: spec.md5.to_vec(),
                file_id: spec.file_id.to_owned(),
                upload_key: spec.upload_key.to_vec(),
            }),
            client: Some(HighwayClientInfo {
                client_type: 3,
                app_id: "100".to_owned(),
                terminal_type: 3,
                client_version: "1.1.1".to_owned(),
                field6: 4,
            }),
            name: Some(HighwayFileName {
                file_name: spec.file_name.to_owned(),
            }),
            host: Some(HighwayHostConfig {
                hosts: vec![HighwayHost {
                    url: Some(HighwayUrl {
                        field1: 1,
                        host: spec.upload_host.to_owned(),
                    }),
                    port: spec.upload_port,
                }],
            }),
        }),
        private_trailer: u32::from(spec.private_trailer),
    }
    .encode_to_vec()
}

#[derive(Clone, PartialEq, Message)]
pub(crate) struct HighwayExtension {
    #[prost(int32, tag = "1")]
    pub(crate) field1: i32,
    #[prost(int32, tag = "2")]
    pub(crate) field2: i32,
    #[prost(message, optional, tag = "100")]
    pub(crate) entry: Option<HighwayEntry>,
    #[prost(uint32, tag = "200")]
    pub(crate) private_trailer: u32,
}

#[derive(Clone, PartialEq, Message)]
pub(crate) struct HighwayEntry {
    #[prost(message, optional, tag = "100")]
    pub(crate) business: Option<HighwayBusiness>,
    #[prost(message, optional, tag = "200")]
    pub(crate) file: Option<HighwayFile>,
    #[prost(message, optional, tag = "300")]
    client: Option<HighwayClientInfo>,
    #[prost(message, optional, tag = "400")]
    name: Option<HighwayFileName>,
    #[prost(message, optional, tag = "500")]
    host: Option<HighwayHostConfig>,
}

#[derive(Clone, PartialEq, Message)]
pub(crate) struct HighwayBusiness {
    #[prost(uint64, tag = "100")]
    pub(crate) sender: u64,
    #[prost(uint64, tag = "200")]
    pub(crate) receiver: u64,
    #[prost(uint64, tag = "400")]
    pub(crate) group: u64,
}

#[derive(Clone, PartialEq, Message)]
pub(crate) struct HighwayFile {
    #[prost(uint64, tag = "100")]
    pub(crate) file_size: u64,
    #[prost(bytes = "vec", tag = "200")]
    pub(crate) md5: Vec<u8>,
    #[prost(bytes = "vec", tag = "300")]
    pub(crate) check_key: Vec<u8>,
    #[prost(bytes = "vec", tag = "400")]
    pub(crate) second_md5: Vec<u8>,
    #[prost(string, tag = "600")]
    pub(crate) file_id: String,
    #[prost(bytes = "vec", tag = "700")]
    pub(crate) upload_key: Vec<u8>,
}

#[derive(Clone, PartialEq, Message)]
struct HighwayClientInfo {
    #[prost(int32, tag = "100")]
    client_type: i32,
    #[prost(string, tag = "200")]
    app_id: String,
    #[prost(int32, tag = "300")]
    terminal_type: i32,
    #[prost(string, tag = "400")]
    client_version: String,
    #[prost(int32, tag = "600")]
    field6: i32,
}

#[derive(Clone, PartialEq, Message)]
struct HighwayFileName {
    #[prost(string, tag = "100")]
    file_name: String,
}

#[derive(Clone, PartialEq, Message)]
struct HighwayHostConfig {
    #[prost(message, repeated, tag = "200")]
    hosts: Vec<HighwayHost>,
}

#[derive(Clone, PartialEq, Message)]
struct HighwayHost {
    #[prost(message, optional, tag = "1")]
    url: Option<HighwayUrl>,
    #[prost(uint32, tag = "2")]
    port: u32,
}

#[derive(Clone, PartialEq, Message)]
struct HighwayUrl {
    #[prost(int32, tag = "1")]
    field1: i32,
    #[prost(string, tag = "2")]
    host: String,
}
