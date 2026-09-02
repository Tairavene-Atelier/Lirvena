//! Closed login state transition tests.

use qq_domain::{LoginFailure, LoginMachine, LoginState};

#[test]
fn qr_login_and_online_revocation_follow_closed_paths() -> Result<(), Box<dyn std::error::Error>> {
    let mut machine = LoginMachine::new();
    machine.transition(LoginState::FetchingQr)?;
    machine.transition(LoginState::AwaitingScan)?;
    machine.transition(LoginState::AwaitingConfirmation)?;
    machine.transition(LoginState::ExchangingCredentials)?;
    machine.transition(LoginState::Registering)?;
    machine.transition(LoginState::Online)?;
    machine.transition(LoginState::ProtectiveOffline)?;
    assert_eq!(machine.state(), LoginState::ProtectiveOffline);
    Ok(())
}

#[test]
fn qr_confirmation_cannot_skip_credential_exchange() -> Result<(), Box<dyn std::error::Error>> {
    let mut machine = LoginMachine::new();
    machine.transition(LoginState::FetchingQr)?;
    machine.transition(LoginState::AwaitingScan)?;
    let error = machine
        .transition(LoginState::Online)
        .err()
        .ok_or("expected rejection")?;
    assert_eq!(error.from(), LoginState::AwaitingScan);
    assert_eq!(error.to(), LoginState::Online);
    assert_eq!(machine.state(), LoginState::AwaitingScan);
    Ok(())
}

#[test]
fn polling_may_observe_confirmation_and_credentials_in_one_step()
-> Result<(), Box<dyn std::error::Error>> {
    let mut machine = LoginMachine::new();
    machine.transition(LoginState::FetchingQr)?;
    machine.transition(LoginState::AwaitingScan)?;
    machine.transition(LoginState::ExchangingCredentials)?;
    assert_eq!(machine.state(), LoginState::ExchangingCredentials);
    Ok(())
}

#[test]
fn failures_are_classified_without_payload_data() -> Result<(), Box<dyn std::error::Error>> {
    let mut machine = LoginMachine::new();
    machine.transition(LoginState::FetchingQr)?;
    machine.transition(LoginState::Failed(LoginFailure::Protocol))?;
    assert_eq!(machine.state(), LoginState::Failed(LoginFailure::Protocol));
    Ok(())
}
