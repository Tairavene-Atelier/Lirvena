use prost::Message;

use crate::{CodecError, TelemetryReportId, proto};

const TELEMETRY_TRANSCRIPT_DOMAIN: &[u8] = b"ceylith-v2-community-telemetry-v1";

/// Coarse group-count bucket disclosed by Community installations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GroupCountBucket {
    /// No groups.
    Zero,
    /// One through five groups.
    OneToFive,
    /// Six through twenty groups.
    SixToTwenty,
    /// Twenty-one through fifty groups.
    TwentyOneToFifty,
    /// Fifty-one through one hundred groups.
    FiftyOneToOneHundred,
    /// One hundred one through two hundred groups.
    OneHundredOneToTwoHundred,
    /// Two hundred one through five hundred groups.
    TwoHundredOneToFiveHundred,
    /// More than five hundred groups.
    OverFiveHundred,
}

impl GroupCountBucket {
    /// Buckets one exact group count without retaining it.
    #[must_use]
    pub const fn from_count(value: u64) -> Self {
        match value {
            0 => Self::Zero,
            1..=5 => Self::OneToFive,
            6..=20 => Self::SixToTwenty,
            21..=50 => Self::TwentyOneToFifty,
            51..=100 => Self::FiftyOneToOneHundred,
            101..=200 => Self::OneHundredOneToTwoHundred,
            201..=500 => Self::TwoHundredOneToFiveHundred,
            _ => Self::OverFiveHundred,
        }
    }

    /// Returns the closed protobuf discriminant.
    #[must_use]
    pub const fn to_wire(self) -> proto::GroupCountBucket {
        match self {
            Self::Zero => proto::GroupCountBucket::Zero,
            Self::OneToFive => proto::GroupCountBucket::OneToFive,
            Self::SixToTwenty => proto::GroupCountBucket::SixToTwenty,
            Self::TwentyOneToFifty => proto::GroupCountBucket::TwentyOneToFifty,
            Self::FiftyOneToOneHundred => proto::GroupCountBucket::FiftyOneToOneHundred,
            Self::OneHundredOneToTwoHundred => proto::GroupCountBucket::OneHundredOneToTwoHundred,
            Self::TwoHundredOneToFiveHundred => proto::GroupCountBucket::TwoHundredOneToFiveHundred,
            Self::OverFiveHundred => proto::GroupCountBucket::OverFiveHundred,
        }
    }
}

/// Coarse daily inbound or outbound message-count bucket.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MessageCountBucket {
    /// No messages.
    Zero,
    /// One through twenty messages.
    OneToTwenty,
    /// Twenty-one through one hundred messages.
    TwentyOneToOneHundred,
    /// One hundred one through five hundred messages.
    OneHundredOneToFiveHundred,
    /// Five hundred one through two thousand messages.
    FiveHundredOneToTwoThousand,
    /// Two thousand one through ten thousand messages.
    TwoThousandOneToTenThousand,
    /// More than ten thousand messages.
    OverTenThousand,
}

impl MessageCountBucket {
    /// Buckets one exact daily message count without retaining it.
    #[must_use]
    pub const fn from_count(value: u64) -> Self {
        match value {
            0 => Self::Zero,
            1..=20 => Self::OneToTwenty,
            21..=100 => Self::TwentyOneToOneHundred,
            101..=500 => Self::OneHundredOneToFiveHundred,
            501..=2_000 => Self::FiveHundredOneToTwoThousand,
            2_001..=10_000 => Self::TwoThousandOneToTenThousand,
            _ => Self::OverTenThousand,
        }
    }

    /// Returns the closed protobuf discriminant.
    #[must_use]
    pub const fn to_wire(self) -> proto::MessageCountBucket {
        match self {
            Self::Zero => proto::MessageCountBucket::Zero,
            Self::OneToTwenty => proto::MessageCountBucket::OneToTwenty,
            Self::TwentyOneToOneHundred => proto::MessageCountBucket::TwentyOneToOneHundred,
            Self::OneHundredOneToFiveHundred => {
                proto::MessageCountBucket::OneHundredOneToFiveHundred
            }
            Self::FiveHundredOneToTwoThousand => {
                proto::MessageCountBucket::FiveHundredOneToTwoThousand
            }
            Self::TwoThousandOneToTenThousand => {
                proto::MessageCountBucket::TwoThousandOneToTenThousand
            }
            Self::OverTenThousand => proto::MessageCountBucket::OverTenThousand,
        }
    }
}

/// Coarse daily active-duration bucket.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActiveDurationBucket {
    /// No active time.
    Zero,
    /// Less than one hour.
    UnderOneHour,
    /// One through four hours.
    OneToFourHours,
    /// Four through eight hours.
    FourToEightHours,
    /// Eight through sixteen hours.
    EightToSixteenHours,
    /// Sixteen through twenty-four hours.
    SixteenToTwentyFourHours,
}

impl ActiveDurationBucket {
    /// Buckets exact active milliseconds into the fixed daily ranges.
    #[must_use]
    pub const fn from_milliseconds(value: u64) -> Self {
        const HOUR: u64 = 60 * 60 * 1_000;
        match value {
            0 => Self::Zero,
            1..HOUR => Self::UnderOneHour,
            HOUR..=14_400_000 => Self::OneToFourHours,
            14_400_001..=28_800_000 => Self::FourToEightHours,
            28_800_001..=57_600_000 => Self::EightToSixteenHours,
            _ => Self::SixteenToTwentyFourHours,
        }
    }

    /// Returns the closed protobuf discriminant.
    #[must_use]
    pub const fn to_wire(self) -> proto::ActiveDurationBucket {
        match self {
            Self::Zero => proto::ActiveDurationBucket::Zero,
            Self::UnderOneHour => proto::ActiveDurationBucket::UnderOneHour,
            Self::OneToFourHours => proto::ActiveDurationBucket::OneToFourHours,
            Self::FourToEightHours => proto::ActiveDurationBucket::FourToEightHours,
            Self::EightToSixteenHours => proto::ActiveDurationBucket::EightToSixteenHours,
            Self::SixteenToTwentyFourHours => proto::ActiveDurationBucket::SixteenToTwentyFourHours,
        }
    }
}

/// Coarse daily account-set change bucket.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AccountChurnBucket {
    /// No account-set changes.
    Zero,
    /// One account-set change.
    One,
    /// Two through three changes.
    TwoToThree,
    /// Four through seven changes.
    FourToSeven,
    /// Eight or more changes.
    EightOrMore,
}

impl AccountChurnBucket {
    /// Buckets one exact account-set change count without retaining it.
    #[must_use]
    pub const fn from_count(value: u64) -> Self {
        match value {
            0 => Self::Zero,
            1 => Self::One,
            2..=3 => Self::TwoToThree,
            4..=7 => Self::FourToSeven,
            _ => Self::EightOrMore,
        }
    }

    /// Returns the closed protobuf discriminant.
    #[must_use]
    pub const fn to_wire(self) -> proto::AccountChurnBucket {
        match self {
            Self::Zero => proto::AccountChurnBucket::Zero,
            Self::One => proto::AccountChurnBucket::One,
            Self::TwoToThree => proto::AccountChurnBucket::TwoToThree,
            Self::FourToSeven => proto::AccountChurnBucket::FourToSeven,
            Self::EightOrMore => proto::AccountChurnBucket::EightOrMore,
        }
    }
}

/// Returns the canonical installation-signature transcript for a telemetry report.
///
/// The signature field is always cleared before encoding, so callers cannot sign a
/// self-referential or alternative representation.
///
/// # Errors
///
/// Returns an error when the report fields violate the closed public contract.
pub fn telemetry_signing_transcript(
    report: &proto::CommunityTelemetryReport,
) -> Result<Vec<u8>, CodecError> {
    validate_report_fields(report, false)?;
    let mut unsigned = report.clone();
    unsigned.installation_signature.clear();
    let mut transcript =
        Vec::with_capacity(TELEMETRY_TRANSCRIPT_DOMAIN.len() + unsigned.encoded_len());
    transcript.extend_from_slice(TELEMETRY_TRANSCRIPT_DOMAIN);
    unsigned
        .encode(&mut transcript)
        .map_err(|_| CodecError::Protobuf)?;
    Ok(transcript)
}

pub(crate) fn validate_report(report: &proto::CommunityTelemetryReport) -> Result<(), CodecError> {
    validate_report_fields(report, true)
}

pub(crate) fn validate_receipt(receipt: &proto::TelemetryReceipt) -> Result<(), CodecError> {
    TelemetryReportId::try_from(receipt.report_id.as_slice())
        .map_err(|_| CodecError::InvalidField)?;
    if !receipt.accepted || receipt.code == 0 {
        return Err(CodecError::InvalidField);
    }
    Ok(())
}

fn validate_report_fields(
    report: &proto::CommunityTelemetryReport,
    require_signature: bool,
) -> Result<(), CodecError> {
    if report.runtime_lease.is_empty()
        || report.utc_day == 0
        || report.generated_at_ms == 0
        || report.profile_id.len() != 16
        || report.profile_manifest_digest.len() != 32
        || report.build_digest.len() != 32
        || report.platform == 0
        || report.architecture == 0
        || (require_signature && report.installation_signature.len() != 64)
        || (!require_signature
            && !report.installation_signature.is_empty()
            && report.installation_signature.len() != 64)
    {
        return Err(CodecError::InvalidField);
    }
    TelemetryReportId::try_from(report.report_id.as_slice())
        .map_err(|_| CodecError::InvalidField)?;
    closed_bucket::<proto::GroupCountBucket>(report.group_count)?;
    closed_bucket::<proto::MessageCountBucket>(report.messages_received)?;
    closed_bucket::<proto::MessageCountBucket>(report.messages_sent)?;
    closed_bucket::<proto::ActiveDurationBucket>(report.active_duration)?;
    closed_bucket::<proto::AccountChurnBucket>(report.account_churn)
}

fn closed_bucket<T>(value: i32) -> Result<(), CodecError>
where
    T: TryFrom<i32>,
{
    if value == 0 || T::try_from(value).is_err() {
        Err(CodecError::InvalidField)
    } else {
        Ok(())
    }
}
