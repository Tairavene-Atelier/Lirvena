use std::collections::BTreeSet;
use std::io;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use zeroize::Zeroizing;

use super::read::{invalid_config, optional_secret_path, read_public_array, read_secret_array};
use super::{AccountConfig, ProcessConfig, device, parse_account_mode};

const MAX_CONFIG_BYTES: u64 = 1_048_576;
const MAX_ACCOUNTS: usize = 128;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FileConfig {
    ceylith: CeylithSection,
    installation: InstallationSection,
    profile: ProfileSection,
    accounts: Vec<AccountSection>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CeylithSection {
    address: SocketAddr,
    noise_public_key_path: PathBuf,
    profile_verifying_key_path: PathBuf,
    token_path: Option<PathBuf>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InstallationSection {
    id_path: PathBuf,
    signing_key_path: PathBuf,
    noise_key_path: PathBuf,
    state_directory: PathBuf,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProfileSection {
    id_path: PathBuf,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AccountSection {
    slot_id_path: PathBuf,
    mode: String,
    device_path: PathBuf,
    qr_output_path: PathBuf,
}

pub(super) fn load(path: &Path) -> Result<ProcessConfig, io::Error> {
    let bytes = read_bounded(path)?;
    let raw: FileConfig =
        serde_json::from_slice(&bytes).map_err(|_| invalid_config("LIRVENA_CONFIG_PATH JSON"))?;
    let base = path.parent().unwrap_or_else(|| Path::new("."));
    if raw.accounts.is_empty() || raw.accounts.len() > MAX_ACCOUNTS {
        return Err(invalid_config("accounts count"));
    }

    let mut ids = BTreeSet::new();
    let mut accounts = Vec::with_capacity(raw.accounts.len());
    for raw_account in raw.accounts {
        let slot_path = resolve(base, &raw_account.slot_id_path);
        let account_slot_id = read_public_array(&slot_path, "account slot identifier")?;
        if !ids.insert(account_slot_id) {
            return Err(invalid_config("duplicate account slot identifier"));
        }
        accounts.push(AccountConfig {
            account_slot_id,
            account_mode: parse_account_mode(&raw_account.mode)?,
            device: device::load_or_generate(&resolve(base, &raw_account.device_path))?,
            qr_output_path: resolve(base, &raw_account.qr_output_path),
        });
    }

    let token_path = raw
        .ceylith
        .token_path
        .as_ref()
        .map(|value| resolve(base, value));
    Ok(ProcessConfig {
        ceylith_address: raw.ceylith.address,
        ceylith_noise_public_key: read_public_array(
            &resolve(base, &raw.ceylith.noise_public_key_path),
            "Ceylith Noise public key",
        )?,
        ceylith_profile_verifying_key: read_public_array(
            &resolve(base, &raw.ceylith.profile_verifying_key_path),
            "Ceylith Profile verifying key",
        )?,
        token: optional_secret_path(token_path.as_deref(), "Ceylith Token")?,
        installation_id: read_public_array(
            &resolve(base, &raw.installation.id_path),
            "installation identifier",
        )?,
        installation_signing_seed: Zeroizing::new(read_secret_array(
            &resolve(base, &raw.installation.signing_key_path),
            "installation signing key",
        )?),
        installation_noise_seed: Zeroizing::new(read_secret_array(
            &resolve(base, &raw.installation.noise_key_path),
            "installation Noise key",
        )?),
        profile_id: read_public_array(&resolve(base, &raw.profile.id_path), "Profile identifier")?,
        state_directory: resolve(base, &raw.installation.state_directory),
        accounts,
    })
}

fn read_bounded(path: &Path) -> Result<Vec<u8>, io::Error> {
    let metadata = std::fs::metadata(path)?;
    if !metadata.is_file() || metadata.len() == 0 || metadata.len() > MAX_CONFIG_BYTES {
        return Err(invalid_config("LIRVENA_CONFIG_PATH size"));
    }
    std::fs::read(path)
}

fn resolve(base: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_owned()
    } else {
        base.join(path)
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::load;

    #[test]
    fn duplicate_account_slots_fail_before_runtime_start() -> Result<(), Box<dyn std::error::Error>>
    {
        let temporary = TempDir::new()?;
        fs::write(temporary.path().join("slot.bin"), [7_u8; 16])?;
        let config_path = temporary.path().join("lirvena.json");
        fs::write(
            &config_path,
            br#"{
  "ceylith": {
    "address": "127.0.0.1:52194",
    "noise_public_key_path": "noise.pub",
    "profile_verifying_key_path": "profile.pub"
  },
  "installation": {
    "id_path": "installation.id",
    "signing_key_path": "installation.signing",
    "noise_key_path": "installation.noise",
    "state_directory": "state"
  },
  "profile": { "id_path": "profile.id" },
  "accounts": [
    {
      "slot_id_path": "slot.bin",
      "mode": "public",
      "device_path": "first-device.json",
      "qr_output_path": "first-qr.png"
    },
    {
      "slot_id_path": "slot.bin",
      "mode": "require_grant",
      "device_path": "second-device.json",
      "qr_output_path": "second-qr.png"
    }
  ]
}"#,
        )?;
        let error = load(&config_path)
            .err()
            .ok_or_else(|| std::io::Error::other("duplicate account slots were accepted"))?;
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
        Ok(())
    }
}
