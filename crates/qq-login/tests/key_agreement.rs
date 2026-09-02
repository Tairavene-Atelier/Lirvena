//! Linux QQ login key-agreement backend tests.

#![cfg(target_os = "linux")]

use openssl::bn::BigNumContext;
use openssl::ec::{EcGroup, EcKey, PointConversionForm};
use openssl::nid::Nid;
use qq_envelope::{decrypt_qq_tea, encrypt_qq_tea};
use qq_login::{LinuxKeyAgreement, QqKeyAgreement};

#[test]
fn linux_backend_accepts_a_valid_secp192k1_peer() -> Result<(), Box<dyn std::error::Error>> {
    let group = EcGroup::from_curve_name(Nid::SECP192K1)?;
    let peer = EcKey::generate(&group)?;
    let mut context = BigNumContext::new()?;
    let peer_public =
        peer.public_key()
            .to_bytes(&group, PointConversionForm::UNCOMPRESSED, &mut context)?;

    let agreement = LinuxKeyAgreement::new(&peer_public)?;
    assert_eq!(agreement.public_key().len(), 25);
    let ciphertext = encrypt_qq_tea(b"login envelope", agreement.tea_key())?;
    assert_eq!(
        decrypt_qq_tea(&ciphertext, agreement.tea_key())?,
        b"login envelope"
    );
    assert!(!format!("{agreement:?}").contains("login envelope"));
    Ok(())
}

#[test]
fn linux_backend_rejects_invalid_peer_encoding() {
    assert!(LinuxKeyAgreement::new(&[4, 1, 2, 3]).is_err());
}

#[test]
fn linux_backend_derives_matching_ephemeral_response_keys() -> Result<(), Box<dyn std::error::Error>>
{
    let group = EcGroup::from_curve_name(Nid::SECP192K1)?;
    let peer = EcKey::generate(&group)?;
    let mut context = BigNumContext::new()?;
    let peer_public =
        peer.public_key()
            .to_bytes(&group, PointConversionForm::UNCOMPRESSED, &mut context)?;
    let left = LinuxKeyAgreement::new(&peer_public)?;
    let right = LinuxKeyAgreement::new(&peer_public)?;

    let left_key = left.derive_response_key(right.public_key())?;
    let right_key = right.derive_response_key(left.public_key())?;
    assert_eq!(left_key.as_bytes(), right_key.as_bytes());
    Ok(())
}
