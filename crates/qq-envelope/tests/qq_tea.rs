//! QQ TEA golden-vector and rejection tests.

mod common;

use qq_envelope::{
    QqTeaError, QqTeaKey, decrypt_qq_tea, encrypt_qq_tea, encrypt_qq_tea_with_padding,
};

use common::decode_hex;

#[test]
fn deterministic_vector_and_round_trip_match() -> Result<(), Box<dyn std::error::Error>> {
    let key = QqTeaKey::new([
        0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee,
        0xff,
    ]);
    let plaintext = b"Lirvena QR boundary";
    let padding = [0x10, 0x20, 0x30, 0x40, 0x50, 0x60];
    let encrypted = encrypt_qq_tea_with_padding(plaintext, &key, &padding)?;
    assert_eq!(
        encrypted,
        [
            0xad, 0xbc, 0x86, 0x9f, 0x53, 0x4e, 0x3b, 0x8b, 0x34, 0x31, 0x81, 0x31, 0x34, 0xcb,
            0xba, 0x7f, 0x02, 0x72, 0xda, 0xe5, 0x2f, 0x1c, 0xe6, 0x07, 0xc4, 0xf8, 0xfc, 0x79,
            0xd9, 0xd4, 0xfc, 0xae,
        ]
    );
    assert_eq!(decrypt_qq_tea(&encrypted, &key)?, plaintext);
    Ok(())
}

#[test]
fn decrypts_frozen_csharp_provider_ciphertext() -> Result<(), Box<dyn std::error::Error>> {
    let key = QqTeaKey::new([
        0x80, 0x81, 0x82, 0x83, 0x84, 0x85, 0x86, 0x87, 0x88, 0x89, 0x8a, 0x8b, 0x8c, 0x8d, 0x8e,
        0x8f,
    ]);
    let ciphertext = decode_hex(
        "986508441246f9e85d877e9ab5ed6778c0118046af23806ed7e192931feb3c75d5952cf7654640a57eb1d291f660656951aba9ecbe71889bc4a8c09311bdef5f2ba6210d82de01ee3a256d1899a8868ebcbaabeae1b84596",
    )?;
    assert_eq!(
        decrypt_qq_tea(&ciphertext, &key)?,
        (0_u8..=72).collect::<Vec<_>>()
    );
    Ok(())
}

#[test]
fn operating_system_entropy_path_round_trips() -> Result<(), Box<dyn std::error::Error>> {
    let key = QqTeaKey::new([7; 16]);
    let ciphertext = encrypt_qq_tea(b"payload", &key)?;
    assert_eq!(decrypt_qq_tea(&ciphertext, &key)?, b"payload");
    Ok(())
}

#[test]
fn malformed_ciphertext_and_padding_are_rejected() -> Result<(), Box<dyn std::error::Error>> {
    let key = QqTeaKey::new([1; 16]);
    assert_eq!(
        encrypt_qq_tea_with_padding(b"x", &key, &[1, 2]),
        Err(QqTeaError::PaddingLength)
    );
    assert_eq!(
        decrypt_qq_tea(&[0; 15], &key),
        Err(QqTeaError::InvalidCiphertext)
    );
    let mut ciphertext = encrypt_qq_tea_with_padding(b"payload", &key, &[1; 10])?;
    let last = ciphertext.len() - 1;
    ciphertext[last] ^= 1;
    assert_eq!(
        decrypt_qq_tea(&ciphertext, &key),
        Err(QqTeaError::InvalidCiphertext)
    );
    Ok(())
}

#[test]
fn key_debug_is_redacted() {
    let key = QqTeaKey::new([0x41; 16]);
    assert!(!format!("{key:?}").contains("41"));
}
