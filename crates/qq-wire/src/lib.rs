//! QQ wire codec boundary for Lirvena.

mod error;
mod oidb;
mod reader;
mod writer;

pub use error::{LengthPrefix, WireError};
pub use oidb::{
    OidbFrameError, OidbRequestFrame, OidbResponseFrame, decode_oidb_request, decode_oidb_response,
    encode_oidb_request,
};
pub use reader::WireReader;
pub use writer::WireWriter;
