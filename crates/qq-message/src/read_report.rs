use prost::Message;

use crate::MessageDecodeError;

const MAX_UID_BYTES: usize = 128;
const MAX_RESPONSE_BYTES: usize = 64 * 1024;

/// Correlation used to mark one retained message as read.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReadReportInput<'a> {
    /// Group message correlation.
    Group {
        /// Numeric group identifier.
        group_uin: u64,
        /// QQ message sequence within the group.
        sequence: u64,
    },
    /// Direct-message correlation.
    Private {
        /// Current peer UID.
        target_uid: &'a str,
        /// Original or accepted QQ timestamp.
        timestamp: u32,
        /// QQ message sequence.
        sequence: u64,
    },
}

/// Encodes one compiled Linux NT read-report request.
///
/// # Errors
///
/// Returns an error for an invalid peer identity or a correlation that does not fit the compiled
/// wire generation.
pub fn encode_read_report(input: ReadReportInput<'_>) -> Result<Vec<u8>, MessageDecodeError> {
    let request = match input {
        ReadReportInput::Group {
            group_uin,
            sequence,
        } => ReadReportRequest {
            group: Some(GroupReadReport {
                group_uin: narrow_nonzero(group_uin)?,
                start_sequence: narrow_nonzero(sequence)?,
            }),
            private: None,
        },
        ReadReportInput::Private {
            target_uid,
            timestamp,
            sequence,
        } => {
            if target_uid.is_empty()
                || target_uid.len() > MAX_UID_BYTES
                || target_uid.chars().any(char::is_control)
                || timestamp == 0
            {
                return Err(MessageDecodeError);
            }
            ReadReportRequest {
                group: None,
                private: Some(PrivateReadReport {
                    target_uid: target_uid.to_owned(),
                    timestamp,
                    start_sequence: narrow_nonzero(sequence)?,
                }),
            }
        }
    };
    Ok(request.encode_to_vec())
}

/// Validates one Linux NT read-report acknowledgement.
///
/// # Errors
///
/// An empty body is the canonical protobuf encoding of result zero and is accepted only after the
/// caller has authenticated and correlated the outer QQ response. A non-empty body must carry an
/// explicit successful result.
///
/// Returns an error when the response is oversized, malformed, lacks a result in a non-empty body,
/// or reports failure.
pub fn validate_read_report_response(input: &[u8]) -> Result<(), MessageDecodeError> {
    if input.len() > MAX_RESPONSE_BYTES {
        return Err(MessageDecodeError);
    }
    if input.is_empty() {
        return Ok(());
    }
    let response = ReadReportResponse::decode(input).map_err(|_error| MessageDecodeError)?;
    if response.result != Some(0) {
        return Err(MessageDecodeError);
    }
    Ok(())
}

fn narrow_nonzero(value: u64) -> Result<u32, MessageDecodeError> {
    u32::try_from(value)
        .ok()
        .filter(|value| *value != 0)
        .ok_or(MessageDecodeError)
}

#[derive(Clone, PartialEq, Message)]
struct ReadReportRequest {
    #[prost(message, optional, tag = "1")]
    group: Option<GroupReadReport>,
    #[prost(message, optional, tag = "2")]
    private: Option<PrivateReadReport>,
}

#[derive(Clone, Copy, PartialEq, Message)]
struct GroupReadReport {
    #[prost(uint32, tag = "1")]
    group_uin: u32,
    #[prost(uint32, tag = "2")]
    start_sequence: u32,
}

#[derive(Clone, PartialEq, Message)]
struct PrivateReadReport {
    #[prost(string, tag = "2")]
    target_uid: String,
    #[prost(uint32, tag = "3")]
    timestamp: u32,
    #[prost(uint32, tag = "4")]
    start_sequence: u32,
}

#[derive(Clone, PartialEq, Message)]
struct ReadReportResponse {
    #[prost(int32, optional, tag = "1")]
    result: Option<i32>,
    #[prost(string, tag = "2")]
    error_message: String,
}
