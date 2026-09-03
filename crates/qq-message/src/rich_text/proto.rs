use prost::Message;

#[derive(Clone, PartialEq, Message)]
pub(super) struct RichTextWire {
    #[prost(bytes = "vec", optional, tag = "1")]
    pub attributes: Option<Vec<u8>>,
    #[prost(bytes = "vec", repeated, tag = "2")]
    pub elements: Vec<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "3")]
    pub file: Option<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "4")]
    pub voice: Option<Vec<u8>>,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct ElementWire {
    #[prost(bytes = "vec", optional, tag = "1")]
    pub text: Option<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "2")]
    pub face: Option<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "4")]
    pub direct_image: Option<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "8")]
    pub group_image: Option<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "12")]
    pub rich_message: Option<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "19")]
    pub video: Option<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "51")]
    pub light_app: Option<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "53")]
    pub common: Option<Vec<u8>>,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct RichMessageWire {
    #[prost(bytes = "vec", tag = "1")]
    pub template: Vec<u8>,
    #[prost(int32, optional, tag = "2")]
    pub service_id: Option<i32>,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct LightAppWire {
    #[prost(bytes = "vec", tag = "1")]
    pub data: Vec<u8>,
    #[prost(bytes = "vec", optional, tag = "2")]
    pub resource_id: Option<Vec<u8>>,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct DirectImageWire {
    #[prost(string, tag = "1")]
    pub name: String,
    #[prost(uint32, tag = "2")]
    pub size: u32,
    #[prost(bytes = "vec", tag = "7")]
    pub digest: Vec<u8>,
    #[prost(uint32, tag = "8")]
    pub height: u32,
    #[prost(uint32, tag = "9")]
    pub width: u32,
    #[prost(string, tag = "15")]
    pub remote_reference: String,
    #[prost(message, optional, tag = "29")]
    pub reserve: Option<DirectImageReserveWire>,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct DirectImageReserveWire {
    #[prost(int32, tag = "1")]
    pub subtype: i32,
    #[prost(string, tag = "8")]
    pub summary: String,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct GroupImageWire {
    #[prost(string, tag = "2")]
    pub name: String,
    #[prost(bytes = "vec", tag = "13")]
    pub digest: Vec<u8>,
    #[prost(string, tag = "16")]
    pub remote_reference: String,
    #[prost(int32, tag = "22")]
    pub width: i32,
    #[prost(int32, tag = "23")]
    pub height: i32,
    #[prost(uint32, tag = "25")]
    pub size: u32,
    #[prost(message, optional, tag = "34")]
    pub reserve: Option<GroupImageReserveWire>,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct GroupImageReserveWire {
    #[prost(int32, tag = "1")]
    pub subtype: i32,
    #[prost(string, tag = "9")]
    pub summary: String,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct LegacyVideoWire {
    #[prost(string, tag = "1")]
    pub uuid: String,
    #[prost(bytes = "vec", tag = "2")]
    pub digest: Vec<u8>,
    #[prost(string, tag = "3")]
    pub name: String,
    #[prost(int32, tag = "5")]
    pub duration_seconds: i32,
    #[prost(int32, tag = "6")]
    pub size: i32,
    #[prost(int32, tag = "7")]
    pub thumbnail_width: i32,
    #[prost(int32, tag = "8")]
    pub thumbnail_height: i32,
    #[prost(int32, tag = "16")]
    pub width: i32,
    #[prost(int32, tag = "17")]
    pub height: i32,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct TextWire {
    #[prost(string, optional, tag = "1")]
    pub text: Option<String>,
    #[prost(bytes = "vec", optional, tag = "3")]
    pub legacy_attributes: Option<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "12")]
    pub reserve: Option<Vec<u8>>,
}

#[derive(Clone, Copy, PartialEq, Message)]
pub(super) struct FaceWire {
    #[prost(int32, optional, tag = "1")]
    pub index: Option<i32>,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct CommonWire {
    #[prost(int32, tag = "1")]
    pub service_type: i32,
    #[prost(bytes = "vec", optional, tag = "2")]
    pub body: Option<Vec<u8>>,
    #[prost(uint32, tag = "3")]
    pub business_type: u32,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct MentionWire {
    #[prost(int32, tag = "3")]
    pub mention_type: i32,
    #[prost(uint32, tag = "4")]
    pub account: u32,
    #[prost(string, tag = "9")]
    pub user: String,
}

#[derive(Clone, Copy, PartialEq, Message)]
pub(super) struct StandardFaceWire {
    #[prost(uint32, tag = "1")]
    pub face_id: u32,
}

#[derive(Clone, Copy, PartialEq, Message)]
pub(super) struct AnimatedFaceWire {
    #[prost(int32, optional, tag = "3")]
    pub face_id: Option<i32>,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct MediaInfoWire {
    #[prost(message, repeated, tag = "1")]
    pub bodies: Vec<MediaBodyWire>,
    #[prost(message, optional, tag = "2")]
    pub extension: Option<MediaExtensionWire>,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct MediaBodyWire {
    #[prost(message, optional, tag = "1")]
    pub index: Option<MediaIndexWire>,
    #[prost(message, optional, tag = "2")]
    pub picture: Option<PictureWire>,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct MediaIndexWire {
    #[prost(message, optional, tag = "1")]
    pub info: Option<MediaFileWire>,
    #[prost(string, tag = "2")]
    pub uuid: String,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct MediaFileWire {
    #[prost(uint32, tag = "1")]
    pub size: u32,
    #[prost(string, tag = "2")]
    pub digest: String,
    #[prost(string, tag = "3")]
    pub sha1: String,
    #[prost(string, tag = "4")]
    pub name: String,
    #[prost(uint32, tag = "6")]
    pub width: u32,
    #[prost(uint32, tag = "7")]
    pub height: u32,
    #[prost(uint32, tag = "8")]
    pub duration_seconds: u32,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct PictureWire {
    #[prost(string, tag = "1")]
    pub remote_reference: String,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct MediaExtensionWire {
    #[prost(message, optional, tag = "1")]
    pub image: Option<ImageExtensionWire>,
}

#[derive(Clone, PartialEq, Message)]
pub(super) struct ImageExtensionWire {
    #[prost(uint32, tag = "1")]
    pub subtype: u32,
    #[prost(string, tag = "2")]
    pub summary: String,
}
