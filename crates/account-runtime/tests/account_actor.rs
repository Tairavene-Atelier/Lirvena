//! Account actor isolation, recovery and durable WAL tests.

use std::path::Path;

use account_runtime::{
    AccountLocalId, AccountPhase, AccountRuntimeConfig, AccountRuntimeError, AccountSupervisor,
    AccountTransition, ProtectiveReason, spawn_account,
};
use rusqlite::Connection;
use tempfile::TempDir;

#[tokio::test(flavor = "current_thread")]
async fn actors_keep_accounts_and_wal_state_independent() -> Result<(), Box<dyn std::error::Error>>
{
    let temporary = TempDir::new()?;
    let first_id = AccountLocalId::from_bytes([0x11; 16]);
    let second_id = AccountLocalId::from_bytes([0x22; 16]);
    let first = spawn_account(config(temporary.path(), first_id)?, 100).await?;
    let second = spawn_account(config(temporary.path(), second_id)?, 100).await?;
    let first_handle = first.handle();
    let second_handle = second.handle();

    let starting = first_handle
        .transition(AccountTransition {
            next: AccountPhase::Starting,
            protective_reason: None,
            occurred_at_ms: 101,
        })
        .await?;
    assert_eq!(starting.current().generation(), 1);
    first_handle
        .transition(AccountTransition {
            next: AccountPhase::Active,
            protective_reason: None,
            occurred_at_ms: 102,
        })
        .await?;

    assert_eq!(first_handle.snapshot().await?.phase(), AccountPhase::Active);
    assert_eq!(
        second_handle.snapshot().await?.phase(),
        AccountPhase::Stopped
    );
    assert_wal(&config(temporary.path(), first_id)?.database_path(), 2)?;
    assert_wal(&config(temporary.path(), second_id)?.database_path(), 0)?;
    first.shutdown().await?;
    second.shutdown().await?;
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn restart_fails_closed_and_new_generation_is_explicit()
-> Result<(), Box<dyn std::error::Error>> {
    let temporary = TempDir::new()?;
    let local_id = AccountLocalId::from_bytes([0x33; 16]);
    let runtime = spawn_account(config(temporary.path(), local_id)?, 200).await?;
    let handle = runtime.handle();
    handle
        .transition(AccountTransition {
            next: AccountPhase::Starting,
            protective_reason: None,
            occurred_at_ms: 201,
        })
        .await?;
    handle
        .transition(AccountTransition {
            next: AccountPhase::Active,
            protective_reason: None,
            occurred_at_ms: 202,
        })
        .await?;
    runtime.shutdown().await?;

    let recovered = spawn_account(config(temporary.path(), local_id)?, 300).await?;
    let recovered_handle = recovered.handle();
    let snapshot = recovered_handle.snapshot().await?;
    assert_eq!(snapshot.phase(), AccountPhase::ProtectiveOffline);
    assert_eq!(
        snapshot.protective_reason(),
        Some(ProtectiveReason::ProcessRestart)
    );
    assert_eq!(snapshot.generation(), 1);
    let receipt = recovered_handle
        .transition(AccountTransition {
            next: AccountPhase::Starting,
            protective_reason: None,
            occurred_at_ms: 301,
        })
        .await?;
    assert_eq!(receipt.current().generation(), 2);
    recovered.shutdown().await?;
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn invalid_transitions_and_capacities_are_rejected() -> Result<(), Box<dyn std::error::Error>>
{
    let temporary = TempDir::new()?;
    let local_id = AccountLocalId::from_bytes([0x44; 16]);
    assert_eq!(
        AccountRuntimeConfig::new(temporary.path().to_path_buf(), local_id, 0).err(),
        Some(AccountRuntimeError::Configuration)
    );
    assert_eq!(
        AccountRuntimeConfig::new(temporary.path().to_path_buf(), local_id, 1_025).err(),
        Some(AccountRuntimeError::Configuration)
    );

    let runtime = spawn_account(config(temporary.path(), local_id)?, 400).await?;
    let error = runtime
        .handle()
        .transition(AccountTransition {
            next: AccountPhase::Active,
            protective_reason: None,
            occurred_at_ms: 401,
        })
        .await
        .err()
        .ok_or_else(|| std::io::Error::other("invalid transition was accepted"))?;
    assert_eq!(error, AccountRuntimeError::TransitionRejected);
    runtime.shutdown().await?;
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn future_schema_is_not_silently_reinterpreted() -> Result<(), Box<dyn std::error::Error>> {
    let temporary = TempDir::new()?;
    let local_id = AccountLocalId::from_bytes([0x55; 16]);
    let config = config(temporary.path(), local_id)?;
    prepare_database_parent(&config.database_path())?;
    let connection = Connection::open(config.database_path())?;
    connection.pragma_update(None, "user_version", 2)?;
    drop(connection);
    let error = spawn_account(config, 500)
        .await
        .err()
        .ok_or_else(|| std::io::Error::other("future schema was accepted"))?;
    assert_eq!(error, AccountRuntimeError::Configuration);
    Ok(())
}

#[cfg(unix)]
#[tokio::test(flavor = "current_thread")]
async fn insecure_existing_state_directory_is_rejected() -> Result<(), Box<dyn std::error::Error>> {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    let temporary = TempDir::new()?;
    let directory = temporary.path().join("insecure");
    fs::create_dir(&directory)?;
    fs::set_permissions(&directory, fs::Permissions::from_mode(0o755))?;
    let config = AccountRuntimeConfig::new(directory, AccountLocalId::from_bytes([0x66; 16]), 8)?;
    let error = spawn_account(config, 600)
        .await
        .err()
        .ok_or_else(|| std::io::Error::other("insecure state directory was accepted"))?;
    assert_eq!(error, AccountRuntimeError::Configuration);
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn supervisor_rejects_duplicates_and_owns_shutdown() -> Result<(), Box<dyn std::error::Error>>
{
    let temporary = TempDir::new()?;
    let local_id = AccountLocalId::from_bytes([0x77; 16]);
    let mut supervisor = AccountSupervisor::new();
    let handle = supervisor
        .spawn(config(temporary.path(), local_id)?, 700)
        .await?;
    assert_eq!(handle.local_id(), local_id);
    assert_eq!(supervisor.len(), 1);
    let duplicate = supervisor
        .spawn(config(temporary.path(), local_id)?, 701)
        .await
        .err()
        .ok_or_else(|| std::io::Error::other("duplicate account was accepted"))?;
    assert_eq!(duplicate, AccountRuntimeError::DuplicateAccount);
    assert_eq!(supervisor.handle(local_id)?.local_id(), local_id);
    supervisor.shutdown_all().await?;
    assert!(supervisor.is_empty());
    assert_eq!(
        supervisor.handle(local_id).err(),
        Some(AccountRuntimeError::UnknownAccount)
    );
    Ok(())
}

fn config(
    directory: &Path,
    local_id: AccountLocalId,
) -> Result<AccountRuntimeConfig, AccountRuntimeError> {
    AccountRuntimeConfig::new(directory.join("accounts"), local_id, 8)
}

fn prepare_database_parent(database_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let parent = database_path
        .parent()
        .ok_or_else(|| std::io::Error::other("database path has no parent"))?;
    std::fs::create_dir_all(parent)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

fn assert_wal(
    database_path: &Path,
    transition_count: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    let connection = Connection::open(database_path)?;
    let journal_mode: String = connection.query_row("PRAGMA journal_mode", [], |row| row.get(0))?;
    let stored_count: u32 =
        connection.query_row("SELECT COUNT(*) FROM account_transitions", [], |row| {
            row.get(0)
        })?;
    assert_eq!(journal_mode.to_ascii_lowercase(), "wal");
    assert_eq!(stored_count, transition_count);
    Ok(())
}
