#![forbid(unsafe_code)]
//! Bounded media acquisition and conversion for Lirvena.

mod error;
mod ffmpeg;
mod image;
mod image_proto;
mod object;
mod record;
mod reference;
mod resolver;
mod rich_request;
mod target;
mod upload;
mod video;

pub use error::MediaError;
pub use ffmpeg::{AudioFormat, FfmpegTranscoder, TranscodePolicy};
pub use image::{
    ImageDescriptor, ImageFormat, ImageMetadataRequest, analyze_image,
    encode_image_metadata_request, parse_image_metadata_response,
};
pub use object::{MediaObject, MediaSourceKind};
pub use record::{
    PreparedRecord, RecordDescriptor, RecordFormat, RecordMetadataRequest,
    encode_record_metadata_request, parse_record_metadata_response, prepare_record,
};
pub use reference::MediaReference;
pub use resolver::{MediaPolicy, MediaResolver, RemoteMediaPolicy};
pub use target::MediaTarget;
pub use upload::RichMediaUploadPlan;
pub use video::{
    VideoDescriptor, VideoMetadataRequest, analyze_video, default_video_thumbnail,
    encode_video_metadata_request, parse_video_metadata_response,
};
