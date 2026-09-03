#![forbid(unsafe_code)]
//! Bounded media acquisition and conversion for Lirvena.

mod error;
mod ffmpeg;
mod image;
mod image_proto;
mod object;
mod reference;
mod resolver;

pub use error::MediaError;
pub use ffmpeg::{AudioFormat, FfmpegTranscoder, TranscodePolicy};
pub use image::{
    ImageDescriptor, ImageFormat, ImageMetadataRequest, ImageTarget, ImageUploadPlan,
    analyze_image, encode_image_metadata_request, parse_image_metadata_response,
};
pub use object::{MediaObject, MediaSourceKind};
pub use reference::MediaReference;
pub use resolver::{MediaPolicy, MediaResolver, RemoteMediaPolicy};
