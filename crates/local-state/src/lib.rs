//! Shared private local `SQLite` policy for Lirvena runtimes.

use core::time::Duration;
use std::ffi::OsStr;
use std::fs;
use std::path::{Component, Path, PathBuf};

use rusqlite::Connection;

/// Redacted local-state configuration or persistence failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LocalStateError {
    /// A path or existing permission boundary was unsafe.
    Configuration,
    /// Directory, file, or `SQLite` setup failed.
    Persistence,
}

impl core::fmt::Display for LocalStateError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::Configuration => "local state configuration rejected",
            Self::Persistence => "local state persistence failed",
        })
    }
}

impl std::error::Error for LocalStateError {}

impl From<rusqlite::Error> for LocalStateError {
    fn from(_error: rusqlite::Error) -> Self {
        Self::Persistence
    }
}

/// Opens one `SQLite` database under a private directory with the shared WAL durability policy.
///
/// The filename must be a single normal path component. Existing Unix directories must already
/// be mode `0700`; database permissions are forced to `0600`.
///
/// # Errors
///
/// Returns an error for an unsafe name or permissions, filesystem failure, or rejected `SQLite`
/// configuration.
pub fn open_private_wal(
    directory: &Path,
    file_name: &OsStr,
) -> Result<(PathBuf, Connection), LocalStateError> {
    validate_file_name(file_name)?;
    ensure_private_directory(directory)?;
    let path = directory.join(file_name);
    reject_existing_symlink(&path)?;
    let connection = Connection::open(&path)?;
    ensure_private_file(&path)?;
    connection.busy_timeout(Duration::from_secs(5))?;
    connection.pragma_update(None, "foreign_keys", true)?;
    connection.pragma_update(None, "synchronous", "FULL")?;
    let mode: String = connection.query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))?;
    if !mode.eq_ignore_ascii_case("wal") {
        return Err(LocalStateError::Persistence);
    }
    Ok((path, connection))
}

/// Reads one existing regular file after enforcing the shared private-file policy.
///
/// Existing symlinks are rejected. On Unix, group and other permission bits must be clear.
///
/// # Errors
///
/// Returns an error when the path is not a private regular file or cannot be read.
pub fn read_private_file(path: &Path) -> Result<Vec<u8>, LocalStateError> {
    reject_existing_symlink(path)?;
    verify_private_file(path)?;
    fs::read(path).map_err(|_error| LocalStateError::Persistence)
}

fn validate_file_name(file_name: &OsStr) -> Result<(), LocalStateError> {
    let path = Path::new(file_name);
    let mut components = path.components();
    if matches!(components.next(), Some(Component::Normal(_))) && components.next().is_none() {
        Ok(())
    } else {
        Err(LocalStateError::Configuration)
    }
}

fn ensure_private_directory(path: &Path) -> Result<(), LocalStateError> {
    if path.exists() {
        reject_existing_symlink(path)?;
        verify_private_directory(path)
    } else {
        fs::create_dir_all(path).map_err(|_error| LocalStateError::Persistence)?;
        set_private_directory(path)
    }
}

fn reject_existing_symlink(path: &Path) -> Result<(), LocalStateError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(LocalStateError::Configuration),
        Ok(_metadata) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_error) => Err(LocalStateError::Persistence),
    }
}

#[cfg(unix)]
fn verify_private_directory(path: &Path) -> Result<(), LocalStateError> {
    use std::os::unix::fs::PermissionsExt;

    let metadata = fs::metadata(path).map_err(|_error| LocalStateError::Persistence)?;
    if metadata.is_dir() && metadata.permissions().mode().trailing_zeros() >= 6 {
        Ok(())
    } else {
        Err(LocalStateError::Configuration)
    }
}

#[cfg(not(unix))]
fn verify_private_directory(path: &Path) -> Result<(), LocalStateError> {
    if fs::metadata(path)
        .map_err(|_error| LocalStateError::Persistence)?
        .is_dir()
    {
        Ok(())
    } else {
        Err(LocalStateError::Configuration)
    }
}

#[cfg(unix)]
fn set_private_directory(path: &Path) -> Result<(), LocalStateError> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
        .map_err(|_error| LocalStateError::Persistence)
}

#[cfg(not(unix))]
fn set_private_directory(path: &Path) -> Result<(), LocalStateError> {
    verify_private_directory(path)
}

#[cfg(unix)]
fn ensure_private_file(path: &Path) -> Result<(), LocalStateError> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .map_err(|_error| LocalStateError::Persistence)
}

#[cfg(unix)]
fn verify_private_file(path: &Path) -> Result<(), LocalStateError> {
    use std::os::unix::fs::PermissionsExt;

    let metadata = fs::metadata(path).map_err(|_error| LocalStateError::Persistence)?;
    if metadata.is_file() && metadata.permissions().mode().trailing_zeros() >= 6 {
        Ok(())
    } else {
        Err(LocalStateError::Configuration)
    }
}

#[cfg(not(unix))]
fn verify_private_file(path: &Path) -> Result<(), LocalStateError> {
    if fs::metadata(path)
        .map_err(|_error| LocalStateError::Persistence)?
        .is_file()
    {
        Ok(())
    } else {
        Err(LocalStateError::Configuration)
    }
}

#[cfg(not(unix))]
fn ensure_private_file(path: &Path) -> Result<(), LocalStateError> {
    fs::metadata(path)
        .map(|_metadata| ())
        .map_err(|_error| LocalStateError::Persistence)
}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;

    #[cfg(unix)]
    use super::read_private_file;
    use super::{LocalStateError, open_private_wal};

    #[test]
    fn database_name_cannot_escape_private_directory() -> Result<(), Box<dyn std::error::Error>> {
        let temporary = tempfile::tempdir()?;
        assert_eq!(
            open_private_wal(temporary.path(), OsStr::new("../outside.sqlite3")).err(),
            Some(LocalStateError::Configuration)
        );
        Ok(())
    }

    #[test]
    fn database_uses_wal_and_foreign_keys() -> Result<(), Box<dyn std::error::Error>> {
        let temporary = tempfile::tempdir()?;
        let directory = temporary.path().join("private");
        let (_path, connection) = open_private_wal(&directory, OsStr::new("state.sqlite3"))?;
        let journal: String = connection.query_row("PRAGMA journal_mode", [], |row| row.get(0))?;
        let foreign_keys: u8 = connection.query_row("PRAGMA foreign_keys", [], |row| row.get(0))?;
        assert_eq!(journal.to_ascii_lowercase(), "wal");
        assert_eq!(foreign_keys, 1);
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn existing_database_symlink_is_rejected() -> Result<(), Box<dyn std::error::Error>> {
        use std::os::unix::fs::symlink;

        let temporary = tempfile::tempdir()?;
        let directory = temporary.path().join("private");
        std::fs::create_dir(&directory)?;
        std::fs::set_permissions(
            &directory,
            <std::fs::Permissions as std::os::unix::fs::PermissionsExt>::from_mode(0o700),
        )?;
        let outside = temporary.path().join("outside.sqlite3");
        std::fs::write(&outside, [])?;
        symlink(&outside, directory.join("state.sqlite3"))?;
        assert_eq!(
            open_private_wal(&directory, OsStr::new("state.sqlite3")).err(),
            Some(LocalStateError::Configuration)
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn private_file_reader_rejects_public_permissions() -> Result<(), Box<dyn std::error::Error>> {
        use std::os::unix::fs::PermissionsExt;

        let temporary = tempfile::tempdir()?;
        let path = temporary.path().join("secret");
        std::fs::write(&path, b"secret")?;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644))?;
        assert_eq!(
            read_private_file(&path).err(),
            Some(LocalStateError::Configuration)
        );
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
        assert_eq!(read_private_file(&path)?, b"secret");
        Ok(())
    }
}
