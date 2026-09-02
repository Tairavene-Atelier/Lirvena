use openssl::bn::BigNumContext;
use openssl::derive::Deriver;
use openssl::ec::{EcGroup, EcKey, EcPoint, PointConversionForm};
use openssl::hash::{MessageDigest, hash};
use openssl::nid::Nid;
use openssl::pkey::{PKey, Private};
use qq_envelope::QqTeaKey;

use crate::{KeyAgreementError, QqKeyAgreement};

const SHARED_VALUE_LEN: usize = 24;

/// OpenSSL-backed secp192k1 agreement for the first Linux QQ profile.
pub struct LinuxKeyAgreement {
    private: EcKey<Private>,
    public_key: Box<[u8]>,
    tea_key: QqTeaKey,
}

impl LinuxKeyAgreement {
    /// Generates an ephemeral key and derives its QQ TEA key from a profile value.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid peer point or a backend failure.
    pub fn new(peer_public: &[u8]) -> Result<Self, KeyAgreementError> {
        let group = EcGroup::from_curve_name(Nid::SECP192K1)
            .map_err(|_error| KeyAgreementError::Backend)?;
        let mut context = BigNumContext::new().map_err(|_error| KeyAgreementError::Backend)?;
        let private = EcKey::generate(&group).map_err(|_error| KeyAgreementError::Backend)?;
        private
            .check_key()
            .map_err(|_error| KeyAgreementError::Backend)?;
        let public_key = private
            .public_key()
            .to_bytes(&group, PointConversionForm::COMPRESSED, &mut context)
            .map_err(|_error| KeyAgreementError::Backend)?;
        let tea_key = derive_tea_key(&private, peer_public)?;
        Ok(Self {
            private,
            public_key: public_key.into_boxed_slice(),
            tea_key,
        })
    }
}

impl QqKeyAgreement for LinuxKeyAgreement {
    fn public_key(&self) -> &[u8] {
        &self.public_key
    }

    fn tea_key(&self) -> &QqTeaKey {
        &self.tea_key
    }

    fn derive_response_key(&self, peer_public: &[u8]) -> Result<QqTeaKey, KeyAgreementError> {
        derive_tea_key(&self.private, peer_public)
    }
}

impl core::fmt::Debug for LinuxKeyAgreement {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("LinuxKeyAgreement")
            .field("public_key_len", &self.public_key.len())
            .field("tea_key", &self.tea_key)
            .finish_non_exhaustive()
    }
}

fn derive(
    private: &EcKey<Private>,
    peer: EcKey<openssl::pkey::Public>,
) -> Result<Vec<u8>, KeyAgreementError> {
    let private =
        PKey::from_ec_key(private.to_owned()).map_err(|_error| KeyAgreementError::Backend)?;
    let peer = PKey::from_ec_key(peer).map_err(|_error| KeyAgreementError::InvalidPeer)?;
    let mut deriver = Deriver::new(&private).map_err(|_error| KeyAgreementError::Backend)?;
    deriver
        .set_peer(&peer)
        .map_err(|_error| KeyAgreementError::InvalidPeer)?;
    deriver
        .derive_to_vec()
        .map_err(|_error| KeyAgreementError::Backend)
}

fn derive_tea_key(
    private: &EcKey<Private>,
    peer_public: &[u8],
) -> Result<QqTeaKey, KeyAgreementError> {
    let group =
        EcGroup::from_curve_name(Nid::SECP192K1).map_err(|_error| KeyAgreementError::Backend)?;
    let mut context = BigNumContext::new().map_err(|_error| KeyAgreementError::Backend)?;
    let peer_point = EcPoint::from_bytes(&group, peer_public, &mut context)
        .map_err(|_error| KeyAgreementError::InvalidPeer)?;
    let peer_key = EcKey::from_public_key(&group, &peer_point)
        .map_err(|_error| KeyAgreementError::InvalidPeer)?;
    peer_key
        .check_key()
        .map_err(|_error| KeyAgreementError::InvalidPeer)?;
    let shared = derive(private, peer_key)?;
    if shared.len() != SHARED_VALUE_LEN {
        return Err(KeyAgreementError::InvalidSharedValue);
    }
    let digest =
        hash(MessageDigest::md5(), &shared).map_err(|_error| KeyAgreementError::Backend)?;
    let tea_bytes: [u8; QqTeaKey::LENGTH] = digest
        .as_ref()
        .try_into()
        .map_err(|_error| KeyAgreementError::InvalidSharedValue)?;
    Ok(QqTeaKey::new(tea_bytes))
}
