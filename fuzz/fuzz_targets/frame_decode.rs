#![no_main]

use ceylith_protocol::{WireLimits, decode_handshake_envelope, decode_secure_frame};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|input: &[u8]| {
    let limits = WireLimits::default();
    let _ = decode_handshake_envelope(input, limits);
    let _ = decode_secure_frame(input, limits);
});
