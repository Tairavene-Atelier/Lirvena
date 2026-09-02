#![no_main]

use ceylith_protocol::{WireLimits, decode_inner_frame};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|input: &[u8]| {
    let _ = decode_inner_frame(input, WireLimits::default());
});
