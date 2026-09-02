use std::io;
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) fn random_nonzero_u32() -> Result<u32, io::Error> {
    for _attempt in 0..16 {
        let value = u32::from_be_bytes(random_array()?);
        if value != 0 {
            return Ok(value);
        }
    }
    Err(io::Error::other("random sequence generation failed"))
}

pub(crate) fn random_array<const N: usize>() -> Result<[u8; N], io::Error> {
    let mut bytes = [0_u8; N];
    getrandom::fill(&mut bytes).map_err(|_| io::Error::other("operating-system entropy failed"))?;
    Ok(bytes)
}

pub(crate) fn now_ms() -> Result<u64, io::Error> {
    let value = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| io::Error::other("system clock precedes Unix epoch"))?
        .as_millis();
    u64::try_from(value).map_err(|_| io::Error::other("system clock overflow"))
}

pub(crate) fn now_seconds() -> Result<u32, io::Error> {
    u32::try_from(now_ms()? / 1_000).map_err(|_| io::Error::other("system clock overflow"))
}

pub(crate) fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}
