use crate::tea::{QqTeaError, QqTeaKey, block};

const BLOCK_LEN: usize = 8;
const TRAILING_ZERO_LEN: usize = 7;
const MAX_PLAINTEXT_LEN: usize = 2 * 1024 * 1024;

/// Encrypts one bounded QQ TEA plaintext with operating-system entropy.
///
/// # Errors
///
/// Returns an error for excessive input, length overflow or unavailable entropy.
pub fn encrypt_qq_tea(plaintext: &[u8], key: &QqTeaKey) -> Result<Vec<u8>, QqTeaError> {
    let padding_len = padding_len(plaintext.len());
    let mut padding = vec![0_u8; padding_len];
    getrandom::fill(&mut padding).map_err(|_error| QqTeaError::Entropy)?;
    encrypt_qq_tea_with_padding(plaintext, key, &padding)
}

/// Encrypts one QQ TEA plaintext with caller-provided deterministic padding.
///
/// This entry point exists for golden-vector and reproducibility tests. Production
/// callers should use [`encrypt_qq_tea`]. The first padding byte is replaced by
/// the QQ padding header.
///
/// # Errors
///
/// Returns an error for excessive input, length overflow or wrong padding length.
pub fn encrypt_qq_tea_with_padding(
    plaintext: &[u8],
    key: &QqTeaKey,
    padding: &[u8],
) -> Result<Vec<u8>, QqTeaError> {
    validate_plaintext(plaintext)?;
    let padding_len = padding_len(plaintext.len());
    if padding.len() != padding_len {
        return Err(QqTeaError::PaddingLength);
    }
    let output_len = padding_len
        .checked_add(plaintext.len())
        .and_then(|length| length.checked_add(TRAILING_ZERO_LEN))
        .ok_or(QqTeaError::LengthOverflow)?;
    let mut output = vec![0_u8; output_len];
    output[..padding_len].copy_from_slice(padding);
    output[0] = u8::try_from(padding_len - 3).map_err(|_error| QqTeaError::LengthOverflow)? | 0xf8;
    output[padding_len..padding_len + plaintext.len()].copy_from_slice(plaintext);
    encrypt_blocks(&mut output, key.words());
    Ok(output)
}

/// Decrypts and strictly validates one bounded QQ TEA ciphertext.
///
/// # Errors
///
/// Returns an error for invalid width, invalid padding or an excessive packet.
pub fn decrypt_qq_tea(ciphertext: &[u8], key: &QqTeaKey) -> Result<Vec<u8>, QqTeaError> {
    if ciphertext.len() < BLOCK_LEN * 2
        || !ciphertext.len().is_multiple_of(BLOCK_LEN)
        || ciphertext.len() > encrypted_bound()
    {
        return Err(QqTeaError::InvalidCiphertext);
    }
    let mut decoded = vec![0_u8; ciphertext.len()];
    decrypt_blocks(ciphertext, &mut decoded, key.words());
    let prefix_len = usize::from(decoded[0] & 7)
        .checked_add(3)
        .ok_or(QqTeaError::LengthOverflow)?;
    let payload_end = decoded
        .len()
        .checked_sub(TRAILING_ZERO_LEN)
        .ok_or(QqTeaError::InvalidCiphertext)?;
    if prefix_len > payload_end || decoded[payload_end..].iter().any(|byte| *byte != 0) {
        return Err(QqTeaError::InvalidCiphertext);
    }
    Ok(decoded[prefix_len..payload_end].to_vec())
}

fn encrypt_blocks(output: &mut [u8], key: [u32; 4]) {
    let mut prior_encrypted = 0_u64;
    let mut prior_mixed = 0_u64;
    for block_bytes in output.chunks_exact_mut(BLOCK_LEN) {
        let plain = read_u64(block_bytes);
        let mixed = plain ^ prior_encrypted;
        let encrypted = block::encrypt(mixed, key) ^ prior_mixed;
        prior_encrypted = encrypted;
        prior_mixed = mixed;
        block_bytes.copy_from_slice(&encrypted.to_be_bytes());
    }
}

fn decrypt_blocks(ciphertext: &[u8], output: &mut [u8], key: [u32; 4]) {
    let mut prior_decrypted = 0_u64;
    let mut prior_ciphertext = 0_u64;
    for (source, destination) in ciphertext
        .chunks_exact(BLOCK_LEN)
        .zip(output.chunks_exact_mut(BLOCK_LEN))
    {
        let encrypted = read_u64(source);
        let decrypted = block::decrypt(prior_decrypted ^ encrypted, key);
        destination.copy_from_slice(&(decrypted ^ prior_ciphertext).to_be_bytes());
        prior_decrypted = decrypted;
        prior_ciphertext = encrypted;
    }
}

fn read_u64(bytes: &[u8]) -> u64 {
    let mut value = [0_u8; BLOCK_LEN];
    value.copy_from_slice(bytes);
    u64::from_be_bytes(value)
}

const fn padding_len(plaintext_len: usize) -> usize {
    10 - ((plaintext_len + 1) & 7)
}

fn validate_plaintext(plaintext: &[u8]) -> Result<(), QqTeaError> {
    if plaintext.len() > MAX_PLAINTEXT_LEN {
        Err(QqTeaError::LengthLimit)
    } else {
        Ok(())
    }
}

const fn encrypted_bound() -> usize {
    MAX_PLAINTEXT_LEN + 16
}
