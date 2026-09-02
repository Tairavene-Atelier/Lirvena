use std::env;
use std::io;
use std::net::SocketAddr;
use std::path::PathBuf;

use account_runtime::AccountGrantMode;
use qq_domain::DeviceProfile;
use zeroize::Zeroizing;

mod device;
mod file;
mod onebot;
mod read;

pub(crate) use onebot::OneBotConfig;

/// Installation-wide process configuration.
pub(super) struct ProcessConfig {
    pub ceylith_address: SocketAddr,
    pub ceylith_noise_public_key: [u8; 32],
    pub ceylith_profile_verifying_key: [u8; 32],
    pub token: Option<Zeroizing<Vec<u8>>>,
    pub installation_id: [u8; 16],
    pub installation_signing_seed: Zeroizing<[u8; 32]>,
    pub installation_noise_seed: Zeroizing<[u8; 32]>,
    pub profile_id: [u8; 16],
    pub state_directory: PathBuf,
    pub accounts: Vec<AccountConfig>,
    pub onebot: Option<OneBotConfig>,
}

/// Configuration owned by one independent QQ account runtime.
#[derive(Clone)]
pub(super) struct AccountConfig {
    pub account_slot_id: [u8; 16],
    pub account_mode: AccountGrantMode,
    pub device: DeviceProfile,
    pub qr_output_path: PathBuf,
}

impl ProcessConfig {
    pub(super) fn from_environment() -> Result<Self, io::Error> {
        match env::var_os("LIRVENA_CONFIG_PATH") {
            Some(path) => file::load(&PathBuf::from(path)),
            None => read::legacy_environment(),
        }
    }
}

pub(super) fn parse_account_mode(value: &str) -> Result<AccountGrantMode, io::Error> {
    match value {
        "public" => Ok(AccountGrantMode::Public),
        "require_grant" => Ok(AccountGrantMode::RequireGrant),
        "allow_public_fallback" => Ok(AccountGrantMode::AllowPublicFallback),
        _ => Err(read::invalid_config("account mode")),
    }
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
