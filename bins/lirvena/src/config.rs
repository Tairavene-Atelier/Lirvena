use std::env;
use std::io;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use zeroize::Zeroizing;

use account_runtime::AccountGrantMode;
use qq_domain::DeviceProfile;

mod device;

pub(super) struct ProcessConfig {
    pub ceylith_address: SocketAddr,
    pub ceylith_noise_public_key: [u8; 32],
    pub ceylith_profile_verifying_key: [u8; 32],
    pub token: Option<Zeroizing<Vec<u8>>>,
    pub installation_id: [u8; 16],
    pub installation_signing_seed: Zeroizing<[u8; 32]>,
    pub installation_noise_seed: Zeroizing<[u8; 32]>,
    pub profile_id: [u8; 16],
    pub account_slot_id: [u8; 16],
    pub account_mode: AccountGrantMode,
    pub state_directory: PathBuf,
    pub device: DeviceProfile,
    pub qr_output_path: PathBuf,
}

impl ProcessConfig {
    pub(super) fn from_environment() -> Result<Self, io::Error> {
        let state_directory = env::var_os("LIRVENA_STATE_DIRECTORY")
            .map_or_else(|| PathBuf::from(".lirvena-state"), PathBuf::from);
        let device_path = env::var_os("LIRVENA_DEVICE_CONFIG_PATH")
            .map_or_else(|| PathBuf::from("device.json"), PathBuf::from);
        let device = device::load_or_generate(&device_path)?;
        Ok(Self {
            ceylith_address: required_env("LIRVENA_CEYLITH_ADDRESS")?
                .parse()
                .map_err(|_| invalid_config("LIRVENA_CEYLITH_ADDRESS"))?,
            ceylith_noise_public_key: read_public_array(
                &required_path("LIRVENA_CEYLITH_NOISE_PUBLIC_KEY_PATH")?,
                "Ceylith Noise public key",
            )?,
            ceylith_profile_verifying_key: read_public_array(
                &required_path("LIRVENA_CEYLITH_PROFILE_VERIFY_KEY_PATH")?,
                "Ceylith Profile verifying key",
            )?,
            token: optional_secret("LIRVENA_TOKEN_PATH", "Ceylith Token")?,
            installation_id: read_public_array(
                &required_path("LIRVENA_INSTALLATION_ID_PATH")?,
                "Installation identifier",
            )?,
            installation_signing_seed: Zeroizing::new(read_secret_array(
                &required_path("LIRVENA_INSTALLATION_SIGNING_KEY_PATH")?,
                "Installation signing key",
            )?),
            installation_noise_seed: Zeroizing::new(read_secret_array(
                &required_path("LIRVENA_INSTALLATION_NOISE_KEY_PATH")?,
                "Installation Noise key",
            )?),
            profile_id: read_public_array(
                &required_path("LIRVENA_PROFILE_ID_PATH")?,
                "Profile identifier",
            )?,
            account_slot_id: read_public_array(
                &required_path("LIRVENA_ACCOUNT_SLOT_ID_PATH")?,
                "Account slot identifier",
            )?,
            account_mode: parse_account_mode(
                &env::var("LIRVENA_ACCOUNT_MODE").unwrap_or_else(|_| String::from("require_grant")),
            )?,
            state_directory,
            device,
            qr_output_path: required_path("LIRVENA_QR_OUTPUT_PATH")?,
        })
    }
}

fn parse_account_mode(value: &str) -> Result<AccountGrantMode, io::Error> {
    match value {
        "public" => Ok(AccountGrantMode::Public),
        "require_grant" => Ok(AccountGrantMode::RequireGrant),
        "allow_public_fallback" => Ok(AccountGrantMode::AllowPublicFallback),
        _ => Err(invalid_config("LIRVENA_ACCOUNT_MODE")),
    }
}

fn optional_secret(
    environment_name: &'static str,
    label: &'static str,
) -> Result<Option<Zeroizing<Vec<u8>>>, io::Error> {
    env::var_os(environment_name)
        .map(PathBuf::from)
        .map(|path| read_secret(&path, label))
        .transpose()
}

fn required_env(name: &'static str) -> Result<String, io::Error> {
    env::var(name).map_err(|_| invalid_config(name))
}

fn required_path(name: &'static str) -> Result<PathBuf, io::Error> {
    required_env(name).map(PathBuf::from)
}

fn invalid_config(name: &'static str) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        format!("required configuration {name} is missing or invalid"),
    )
}

fn read_public_array<const N: usize>(
    path: &Path,
    label: &'static str,
) -> Result<[u8; N], io::Error> {
    std::fs::read(path)?.as_slice().try_into().map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{label} file must contain exactly {N} raw bytes"),
        )
    })
}

fn read_secret_array<const N: usize>(
    path: &Path,
    label: &'static str,
) -> Result<[u8; N], io::Error> {
    let bytes = read_secret(path, label)?;
    bytes.as_slice().try_into().map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{label} file must contain exactly {N} raw bytes"),
        )
    })
}

fn read_secret(path: &Path, label: &'static str) -> Result<Zeroizing<Vec<u8>>, io::Error> {
    local_state::read_private_file(path)
        .map(Zeroizing::new)
        .map_err(|_| {
            io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("{label} file is missing, unsafe, or unreadable"),
            )
        })
}

#[cfg(test)]
mod tests {
    use account_runtime::AccountGrantMode;

    use super::parse_account_mode;

    #[test]
    fn account_modes_are_closed_and_explicit() {
        assert_eq!(
            parse_account_mode("public").ok(),
            Some(AccountGrantMode::Public)
        );
        assert_eq!(
            parse_account_mode("require_grant").ok(),
            Some(AccountGrantMode::RequireGrant)
        );
        assert_eq!(
            parse_account_mode("allow_public_fallback").ok(),
            Some(AccountGrantMode::AllowPublicFallback)
        );
        assert!(parse_account_mode("automatic").is_err());
    }
}
