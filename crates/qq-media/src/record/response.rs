use crate::{MediaError, MediaTarget, RichMediaUploadPlan};

/// Parses one bounded record metadata response.
///
/// # Errors
///
/// Returns an error for rejected, malformed, incomplete, or unbounded material.
pub fn parse_record_metadata_response(
    input: &[u8],
    target: &MediaTarget<'_>,
) -> Result<RichMediaUploadPlan, MediaError> {
    RichMediaUploadPlan::parse(
        input,
        match target {
            MediaTarget::Direct(_) => 1_007,
            MediaTarget::Group(_) => 1_008,
        },
    )
}
