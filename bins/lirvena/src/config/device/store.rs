use std::fmt::Write as FmtWrite;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use qq_domain::DeviceProfile;

use super::{invalid, schema};

const MAX_DEVICE_FILE_BYTES: u64 = 8 * 1024;

pub(super) fn load(path: &Path) -> Result<DeviceProfile, io::Error> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.len() > MAX_DEVICE_FILE_BYTES
    {
        return Err(invalid());
    }
    schema::decode(&fs::read(path)?)
}

pub(super) fn create(path: &Path, profile: &DeviceProfile) -> Result<(), io::Error> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        && !parent.is_dir()
    {
        return Err(invalid());
    }
    let temporary = TemporaryPath::new(temporary_path(path)?);
    let mut file = create_new_file(temporary.path())?;
    file.write_all(&schema::encode(profile)?)?;
    file.sync_all()?;
    drop(file);
    fs::hard_link(temporary.path(), path)?;
    temporary.remove()
}

struct TemporaryPath {
    path: PathBuf,
    remove_on_drop: bool,
}

impl TemporaryPath {
    const fn new(path: PathBuf) -> Self {
        Self {
            path,
            remove_on_drop: true,
        }
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn remove(mut self) -> Result<(), io::Error> {
        fs::remove_file(&self.path)?;
        self.remove_on_drop = false;
        Ok(())
    }
}

impl Drop for TemporaryPath {
    fn drop(&mut self) {
        if self.remove_on_drop {
            let _result = fs::remove_file(&self.path);
        }
    }
}

fn temporary_path(path: &Path) -> Result<PathBuf, io::Error> {
    let name = path.file_name().ok_or_else(invalid)?;
    let mut random = [0_u8; 8];
    getrandom::fill(&mut random).map_err(|_error| io::Error::other("device generation failed"))?;
    let mut suffix = String::with_capacity(16);
    for value in random {
        write!(&mut suffix, "{value:02x}").map_err(|_error| invalid())?;
    }
    Ok(path.with_file_name(format!("{}.tmp-{suffix}", name.to_string_lossy())))
}

#[cfg(unix)]
fn create_new_file(path: &Path) -> Result<fs::File, io::Error> {
    use std::os::unix::fs::OpenOptionsExt;

    OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
}

#[cfg(not(unix))]
fn create_new_file(path: &Path) -> Result<fs::File, io::Error> {
    OpenOptions::new().write(true).create_new(true).open(path)
}
