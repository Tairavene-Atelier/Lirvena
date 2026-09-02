use core::fmt;

use ceylith_protocol::{OpaqueSlot, OpaqueSlotId, OpaqueSlots, ProfileId};
use qq_wire::{LengthPrefix, WireReader, WireWriter};

use crate::linux::{MAX_LOGIN_SDK_LEN, MAX_OS_LEN, MAX_PACKAGE_LEN, MAX_VERSION_LEN};
use crate::{LinuxNtProfile, LinuxNtProfileSpec};

const MAGIC: [u8; 4] = *b"LQPF";
const VERSION: u16 = 3;
const MAX_MANIFEST_LEN: usize = 32 * 1024;

/// Rejected canonical Linux Profile manifest without embedded field data.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProfileManifestError;

impl fmt::Display for ProfileManifestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("QQ profile manifest rejected")
    }
}

impl std::error::Error for ProfileManifestError {}

/// Encodes one validated Linux Profile into its unique signed manifest form.
///
/// # Errors
///
/// Returns an error if the complete manifest exceeds its compiled public bound.
pub fn encode_linux_manifest(profile: &LinuxNtProfile) -> Result<Vec<u8>, ProfileManifestError> {
    let mut writer = WireWriter::new(MAX_MANIFEST_LEN);
    writer.put_bytes(&MAGIC).map_err(map_wire)?;
    writer.put_u16(VERSION).map_err(map_wire)?;
    writer
        .put_bytes(profile.profile_id().as_bytes())
        .map_err(map_wire)?;
    put_text(&mut writer, profile.client_version())?;
    writer.put_u32(profile.app_id()).map_err(map_wire)?;
    writer.put_u32(profile.sub_app_id()).map_err(map_wire)?;
    writer.put_u32(profile.qr_app_id()).map_err(map_wire)?;
    writer
        .put_u16(profile.app_client_version())
        .map_err(map_wire)?;
    put_text(&mut writer, profile.package_name())?;
    put_text(&mut writer, profile.operating_system())?;
    put_text(&mut writer, profile.pt_version())?;
    writer.put_u32(profile.sso_version()).map_err(map_wire)?;
    writer.put_u32(profile.misc_bitmap()).map_err(map_wire)?;
    put_text(&mut writer, profile.login_sdk())?;
    writer.put_u32(profile.main_sig_map()).map_err(map_wire)?;
    writer.put_u32(profile.sub_sig_map()).map_err(map_wire)?;
    writer
        .put_u32(profile.login_misc_bitmap())
        .map_err(map_wire)?;
    writer.put_u32(profile.runtime_abi()).map_err(map_wire)?;
    writer
        .put_u16(u16::try_from(profile.opaque_slots().len()).map_err(|_| ProfileManifestError)?)
        .map_err(map_wire)?;
    for slot in profile.opaque_slots().iter() {
        writer.put_u32(slot.id().get()).map_err(map_wire)?;
        writer
            .put_prefixed_bytes(LengthPrefix::U16Payload, slot.value())
            .map_err(map_wire)?;
    }
    Ok(writer.finish())
}

/// Decodes and revalidates one canonical signed Linux Profile manifest.
///
/// # Errors
///
/// Returns an error for malformed, non-canonical, excessive or invalid fields.
pub fn decode_linux_manifest(bytes: &[u8]) -> Result<LinuxNtProfile, ProfileManifestError> {
    if bytes.len() > MAX_MANIFEST_LEN {
        return Err(ProfileManifestError);
    }
    let mut reader = WireReader::new(bytes);
    if reader.read_bytes(MAGIC.len()).map_err(map_wire)? != MAGIC
        || reader.read_u16().map_err(map_wire)? != VERSION
    {
        return Err(ProfileManifestError);
    }
    let profile_id = ProfileId::try_from(reader.read_bytes(ProfileId::LENGTH).map_err(map_wire)?)
        .map_err(|_| ProfileManifestError)?;
    let spec = LinuxNtProfileSpec {
        profile_id,
        client_version: read_text(&mut reader, MAX_VERSION_LEN)?,
        app_id: reader.read_u32().map_err(map_wire)?,
        sub_app_id: reader.read_u32().map_err(map_wire)?,
        qr_app_id: reader.read_u32().map_err(map_wire)?,
        app_client_version: reader.read_u16().map_err(map_wire)?,
        package_name: read_text(&mut reader, MAX_PACKAGE_LEN)?,
        operating_system: read_text(&mut reader, MAX_OS_LEN)?,
        pt_version: read_text(&mut reader, MAX_VERSION_LEN)?,
        sso_version: reader.read_u32().map_err(map_wire)?,
        misc_bitmap: reader.read_u32().map_err(map_wire)?,
        login_sdk: read_text(&mut reader, MAX_LOGIN_SDK_LEN)?,
        main_sig_map: reader.read_u32().map_err(map_wire)?,
        sub_sig_map: reader.read_u32().map_err(map_wire)?,
        login_misc_bitmap: reader.read_u32().map_err(map_wire)?,
        runtime_abi: reader.read_u32().map_err(map_wire)?,
    };
    let count = usize::from(reader.read_u16().map_err(map_wire)?);
    let mut slots = Vec::with_capacity(count);
    for _index in 0..count {
        let id = OpaqueSlotId::new(reader.read_u32().map_err(map_wire)?)
            .map_err(|_| ProfileManifestError)?;
        let value = reader
            .read_prefixed_bytes(
                LengthPrefix::U16Payload,
                ceylith_protocol::MAX_OPAQUE_SLOT_LEN,
            )
            .map_err(map_wire)?
            .to_vec();
        slots.push(OpaqueSlot::new(id, value).map_err(|_| ProfileManifestError)?);
    }
    reader.finish().map_err(map_wire)?;
    let slots = OpaqueSlots::new(slots).map_err(|_| ProfileManifestError)?;
    let profile = LinuxNtProfile::new(spec, slots).map_err(|_| ProfileManifestError)?;
    if encode_linux_manifest(&profile)? != bytes {
        return Err(ProfileManifestError);
    }
    Ok(profile)
}

fn put_text(writer: &mut WireWriter, value: &str) -> Result<(), ProfileManifestError> {
    writer
        .put_prefixed_bytes(LengthPrefix::U16Payload, value.as_bytes())
        .map_err(map_wire)
}

fn read_text(reader: &mut WireReader<'_>, maximum: usize) -> Result<String, ProfileManifestError> {
    let bytes = reader
        .read_prefixed_bytes(LengthPrefix::U16Payload, maximum)
        .map_err(map_wire)?;
    core::str::from_utf8(bytes)
        .map(str::to_owned)
        .map_err(|_| ProfileManifestError)
}

fn map_wire(_: qq_wire::WireError) -> ProfileManifestError {
    ProfileManifestError
}
