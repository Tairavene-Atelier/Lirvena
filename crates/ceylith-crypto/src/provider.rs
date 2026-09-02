use noise_protocol::{CipherState, DH, HandshakeState, U8Array, patterns};
use noise_rust_crypto::{Blake2s, ChaCha20Poly1305, sensitive::Sensitive};
use x25519_dalek::{PublicKey, StaticSecret};
use zeroize::Zeroizing;

pub(crate) type IkState = HandshakeState<CheckedX25519, ChaCha20Poly1305, Blake2s>;
pub(crate) type TransportCipher = CipherState<ChaCha20Poly1305>;

pub(crate) const PROLOGUE: &[u8] = b"Ceylith secure channel v2";
pub(crate) const NOISE_TAG_LEN: usize = 16;
pub(crate) const MAX_NOISE_MESSAGE_LEN: usize = 65_535;

pub(crate) enum CheckedX25519 {}

impl DH for CheckedX25519 {
    type Key = Sensitive<[u8; 32]>;
    type Pubkey = [u8; 32];
    type Output = Sensitive<[u8; 32]>;

    fn name() -> &'static str {
        "25519"
    }

    fn genkey() -> Self::Key {
        let mut bytes = Zeroizing::new([0_u8; 32]);
        if getrandom::fill(bytes.as_mut()).is_err() {
            std::process::abort();
        }
        Sensitive::from_slice(bytes.as_ref())
    }

    fn pubkey(key: &Self::Key) -> Self::Pubkey {
        let secret = StaticSecret::from(**key);
        *PublicKey::from(&secret).as_bytes()
    }

    fn dh(key: &Self::Key, public: &Self::Pubkey) -> Result<Self::Output, ()> {
        let secret = StaticSecret::from(**key);
        let shared = secret.diffie_hellman(&PublicKey::from(*public));
        if shared.was_contributory() {
            Ok(Sensitive::from_slice(shared.as_bytes()))
        } else {
            Err(())
        }
    }
}

pub(crate) fn initiator_state(
    static_key: Sensitive<[u8; 32]>,
    ephemeral_key: Sensitive<[u8; 32]>,
    remote_static: [u8; 32],
) -> IkState {
    HandshakeState::new(
        patterns::noise_ik(),
        true,
        PROLOGUE,
        Some(static_key),
        Some(ephemeral_key),
        Some(remote_static),
        None,
    )
}

pub(crate) fn responder_state(
    static_key: Sensitive<[u8; 32]>,
    ephemeral_key: Sensitive<[u8; 32]>,
) -> IkState {
    HandshakeState::new(
        patterns::noise_ik(),
        false,
        PROLOGUE,
        Some(static_key),
        Some(ephemeral_key),
        None,
        None,
    )
}
