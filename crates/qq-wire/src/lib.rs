//! QQ wire codec boundary for Lirvena.

mod error;
mod reader;
mod writer;

pub use error::{LengthPrefix, WireError};
pub use reader::WireReader;
pub use writer::WireWriter;
