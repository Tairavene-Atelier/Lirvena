#![forbid(unsafe_code)]
//! Bounded media acquisition and conversion for Lirvena.

mod error;
mod ffmpeg;
mod object;
mod reference;
mod resolver;

pub use error::MediaError;
pub use ffmpeg::{AudioFormat, FfmpegTranscoder, TranscodePolicy};
pub use object::{MediaObject, MediaSourceKind};
pub use reference::MediaReference;
pub use resolver::{MediaPolicy, MediaResolver, RemoteMediaPolicy};
