//! QR artifact and closed polling flow tests.

use qq_domain::{LoginMachine, LoginState};
use qq_login::{
    QrArtifact, QrArtifactError, QrControlEvent, QrPollState, accept_qr_artifact, apply_qr_poll,
    begin_qr_request,
};

fn artifact() -> Result<QrArtifact, QrArtifactError> {
    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    png.extend_from_slice(b"bounded-test-payload");
    QrArtifact::new(
        "https://example.invalid/qr?id=7".to_owned(),
        png,
        1_000,
        120,
    )
}

#[test]
fn artifact_is_redacted_and_renders_terminal_matrix() -> Result<(), Box<dyn std::error::Error>> {
    let artifact = artifact()?;
    let debug = format!("{artifact:?}");
    assert!(!debug.contains("example.invalid"));
    assert!(artifact.terminal_text()?.contains('█'));
    assert!(!artifact.is_expired_at(120_999));
    assert!(artifact.is_expired_at(121_000));
    Ok(())
}

#[test]
fn poll_flow_emits_artifact_and_state_events() -> Result<(), Box<dyn std::error::Error>> {
    let artifact = artifact()?;
    let mut machine = LoginMachine::new();
    assert!(matches!(
        begin_qr_request(&mut machine, 900)?,
        QrControlEvent::StateChanged {
            current: LoginState::FetchingQr,
            ..
        }
    ));
    assert!(matches!(
        accept_qr_artifact(&mut machine, &artifact, 1_000)?,
        QrControlEvent::Available { .. }
    ));
    apply_qr_poll(&mut machine, QrPollState::WaitingForConfirmation, 2_000)?;
    apply_qr_poll(&mut machine, QrPollState::Confirmed, 3_000)?;
    assert_eq!(machine.state(), LoginState::ExchangingCredentials);
    Ok(())
}

#[test]
fn polling_codes_are_closed() -> Result<(), Box<dyn std::error::Error>> {
    assert_eq!(QrPollState::try_from(0)?, QrPollState::Confirmed);
    assert_eq!(QrPollState::try_from(17)?, QrPollState::Expired);
    assert_eq!(QrPollState::try_from(48)?, QrPollState::WaitingForScan);
    assert_eq!(
        QrPollState::try_from(53)?,
        QrPollState::WaitingForConfirmation
    );
    assert_eq!(QrPollState::try_from(54)?, QrPollState::Canceled);
    let unknown = QrPollState::try_from(99)
        .err()
        .ok_or("expected unknown state")?;
    assert_eq!(unknown.value(), 99);
    Ok(())
}
