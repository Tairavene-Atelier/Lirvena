use std::io;
use std::path::Path;

use qq_domain::DeviceProfile;

mod generator;
mod schema;
mod store;

pub(super) fn load_or_generate(path: &Path) -> Result<DeviceProfile, io::Error> {
    match store::load(path) {
        Ok(profile) => Ok(profile),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let profile = generator::generate()?;
            match store::create(path, &profile) {
                Ok(()) => Ok(profile),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => store::load(path),
                Err(error) => Err(error),
            }
        }
        Err(error) => Err(error),
    }
}

fn invalid() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        "device.json contains an invalid or Ceylith-managed field",
    )
}

#[cfg(test)]
mod tests;
