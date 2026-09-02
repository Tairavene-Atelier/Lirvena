use ceylith_protocol::{
    AccountChurnBucket, ActiveDurationBucket, Digest32, GroupCountBucket, MessageCountBucket,
    ProfileId, SessionAdmission, TelemetryReportId, proto, telemetry_signing_transcript,
};
use ed25519_dalek::{Signer, SigningKey};
use zeroize::Zeroizing;

use crate::ClientError;

/// Exact values already reduced to the approved Community daily buckets.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CommunityTelemetrySpec {
    /// Idempotency key for this installation/day report.
    pub report_id: TelemetryReportId,
    /// UTC day number since the Unix epoch.
    pub utc_day: u32,
    /// Coarse current group count.
    pub group_count: GroupCountBucket,
    /// Coarse received-message count for the day.
    pub messages_received: MessageCountBucket,
    /// Coarse sent-message count for the day.
    pub messages_sent: MessageCountBucket,
    /// Coarse active duration for the day.
    pub active_duration: ActiveDurationBucket,
    /// Negotiated public Profile identifier.
    pub profile_id: ProfileId,
    /// Digest of the accepted signed Profile manifest.
    pub profile_manifest_digest: Digest32,
    /// Digest of the running Lirvena build.
    pub build_digest: Digest32,
    /// Closed public platform code from the runtime descriptor.
    pub platform: u32,
    /// Closed public architecture code from the runtime descriptor.
    pub architecture: u32,
    /// Coarse account-set churn for the day.
    pub account_churn: AccountChurnBucket,
    /// Generation time in Unix milliseconds.
    pub generated_at_ms: u64,
}

/// Installation-bound signer for Community telemetry only.
pub struct CommunityTelemetrySigner {
    signing_key: SigningKey,
}

impl CommunityTelemetrySigner {
    /// Imports the same protected installation signing seed used by the session identity.
    #[must_use]
    pub fn from_seed(seed: [u8; 32]) -> Self {
        let seed = Zeroizing::new(seed);
        Self {
            signing_key: SigningKey::from_bytes(&seed),
        }
    }

    /// Builds and signs a report bound to the authenticated runtime lease.
    ///
    /// # Errors
    ///
    /// Returns an error unless the admission is Community or fields violate the wire contract.
    pub fn report(
        &self,
        admission: &SessionAdmission,
        spec: CommunityTelemetrySpec,
    ) -> Result<proto::InnerFrame, ClientError> {
        if admission.grant_class() != ceylith_protocol::GrantClass::Community {
            return Err(ClientError::Protocol);
        }
        let mut report = proto::CommunityTelemetryReport {
            runtime_lease: admission.runtime_lease().to_vec(),
            report_id: spec.report_id.as_bytes().to_vec(),
            utc_day: spec.utc_day,
            group_count: spec.group_count.to_wire() as i32,
            messages_received: spec.messages_received.to_wire() as i32,
            messages_sent: spec.messages_sent.to_wire() as i32,
            active_duration: spec.active_duration.to_wire() as i32,
            profile_id: spec.profile_id.as_bytes().to_vec(),
            profile_manifest_digest: spec.profile_manifest_digest.as_bytes().to_vec(),
            build_digest: spec.build_digest.as_bytes().to_vec(),
            platform: spec.platform,
            architecture: spec.architecture,
            account_churn: spec.account_churn.to_wire() as i32,
            generated_at_ms: spec.generated_at_ms,
            installation_signature: Vec::new(),
        };
        let transcript = Zeroizing::new(telemetry_signing_transcript(&report)?);
        report.installation_signature = self.signing_key.sign(&transcript).to_bytes().to_vec();
        Ok(proto::InnerFrame {
            contract: ceylith_protocol::CURRENT_INNER_CONTRACT,
            body: Some(proto::inner_frame::Body::CommunityTelemetryReport(report)),
        })
    }
}

impl core::fmt::Debug for CommunityTelemetrySigner {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("CommunityTelemetrySigner([REDACTED])")
    }
}

/// Verifies a successful receipt and returns its report id.
///
/// # Errors
///
/// Returns an error for the wrong response body, rejection or correlation mismatch.
pub fn decode_telemetry_receipt(
    frame: &proto::InnerFrame,
    expected: TelemetryReportId,
) -> Result<TelemetryReportId, ClientError> {
    let Some(proto::inner_frame::Body::TelemetryReceipt(receipt)) = frame.body.as_ref() else {
        return Err(ClientError::Protocol);
    };
    let report_id = TelemetryReportId::try_from(receipt.report_id.as_slice())
        .map_err(|_| ClientError::Protocol)?;
    if !receipt.accepted || receipt.code == 0 || report_id != expected {
        return Err(ClientError::Protocol);
    }
    Ok(report_id)
}
