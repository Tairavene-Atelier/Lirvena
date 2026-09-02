//! QQ TEA golden-vector and rejection tests.

use qq_envelope::{
    QqTeaError, QqTeaKey, decrypt_qq_tea, encrypt_qq_tea, encrypt_qq_tea_with_padding,
};

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
