use core::fmt;

use prost::Message;

const MAX_OIDB_BODY_BYTES: usize = 2 * 1024 * 1024;
const MAX_ERROR_MESSAGE_BYTES: usize = 4 * 1024;

/// Validated generic OIDB request envelope.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OidbRequestFrame {
    command: u32,
    subcommand: u32,
    body: Vec<u8>,
    reserved: i32,
}

impl OidbRequestFrame {
    /// Returns the numeric OIDB command.
    #[must_use]
    pub const fn command(&self) -> u32 {
        self.command
    }

    /// Returns the numeric OIDB subcommand.
    #[must_use]
    pub const fn subcommand(&self) -> u32 {
        self.subcommand
    }

    /// Returns the bounded encoded inner body.
    #[must_use]
    pub fn body(&self) -> &[u8] {
        &self.body
    }

    /// Returns the profile-selected outer reservation value.
    #[must_use]
    pub const fn reserved(&self) -> i32 {
        self.reserved
    }
}

/// Validated generic OIDB response envelope.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OidbResponseFrame {
    error_code: u32,
    body: Vec<u8>,
}

impl OidbResponseFrame {
    /// Returns QQ's outer result code.
    #[must_use]
    pub const fn error_code(&self) -> u32 {
        self.error_code
    }

    /// Returns the bounded encoded inner response body.
    #[must_use]
    pub fn body(&self) -> &[u8] {
        &self.body
    }
}

/// Encodes one generic OIDB request without duplicating its outer protobuf shape.
///
/// # Errors
///
/// Returns an error for a zero command or an empty or oversized inner body.
pub fn encode_oidb_request(
    command: u32,
    subcommand: u32,
    body: &[u8],
    reserved: i32,
) -> Result<Vec<u8>, OidbFrameError> {
    validate_command_and_body(command, body)?;
    Ok(OidbWire {
        command,
        subcommand,
        error_code: 0,
        body: body.to_vec(),
        error_message: String::new(),
        reserved,
    }
    .encode_to_vec())
}

/// Decodes and validates one generic OIDB request.
///
/// # Errors
///
/// Returns an error for malformed, rejected, or unbounded fields.
pub fn decode_oidb_request(input: &[u8]) -> Result<OidbRequestFrame, OidbFrameError> {
    let frame = decode(input)?;
    validate_command_and_body(frame.command, &frame.body)?;
    if frame.error_code != 0 || !frame.error_message.is_empty() {
        return Err(OidbFrameError);
    }
    Ok(OidbRequestFrame {
        command: frame.command,
        subcommand: frame.subcommand,
        body: frame.body,
        reserved: frame.reserved,
    })
}

/// Decodes one bounded generic OIDB response.
///
/// # Errors
///
/// Returns an error for malformed or unbounded fields.
pub fn decode_oidb_response(input: &[u8]) -> Result<OidbResponseFrame, OidbFrameError> {
    let frame = decode(input)?;
    if frame.body.len() > MAX_OIDB_BODY_BYTES
        || frame.error_message.len() > MAX_ERROR_MESSAGE_BYTES
        || frame.error_message.chars().any(char::is_control)
    {
        return Err(OidbFrameError);
    }
    Ok(OidbResponseFrame {
        error_code: frame.error_code,
        body: frame.body,
    })
}

fn decode(input: &[u8]) -> Result<OidbWire, OidbFrameError> {
    if input.len() > MAX_OIDB_BODY_BYTES + MAX_ERROR_MESSAGE_BYTES + 256 {
        return Err(OidbFrameError);
    }
    OidbWire::decode(input).map_err(|_error| OidbFrameError)
}

fn validate_command_and_body(command: u32, body: &[u8]) -> Result<(), OidbFrameError> {
    if command == 0 || body.is_empty() || body.len() > MAX_OIDB_BODY_BYTES {
        Err(OidbFrameError)
    } else {
        Ok(())
    }
}

#[derive(Clone, PartialEq, Message)]
struct OidbWire {
    #[prost(uint32, tag = "1")]
    command: u32,
    #[prost(uint32, tag = "2")]
    subcommand: u32,
    #[prost(uint32, tag = "3")]
    error_code: u32,
    #[prost(bytes = "vec", tag = "4")]
    body: Vec<u8>,
    #[prost(string, tag = "5")]
    error_message: String,
    #[prost(int32, tag = "12")]
    reserved: i32,
}

/// Redacted OIDB envelope error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OidbFrameError;

impl fmt::Display for OidbFrameError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("QQ OIDB envelope rejected")
    }
}

impl std::error::Error for OidbFrameError {}
