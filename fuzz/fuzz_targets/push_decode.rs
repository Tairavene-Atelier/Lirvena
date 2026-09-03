#![no_main]

use libfuzzer_sys::fuzz_target;
use qq_message::{
    MessageDecoder, MessageDisposition, decode_group_reaction, decode_rich_text,
    parse_long_message_receive, parse_long_message_send,
};
use qq_online::{OnlineSyncState, PushProcessor};
use qq_profile::{PushBehavior, PushPlanEntry};

fuzz_target!(|data: &[u8]| {
    let (selector, body) = data.split_first().unwrap_or((&0, &[]));
    if selector & 8 != 0 {
        if selector & 1 == 0 {
            let _result = parse_long_message_receive(body);
        } else {
            let _result = parse_long_message_send(body);
        }
        return;
    }
    if selector & 4 != 0 {
        let _result = decode_rich_text(body);
        return;
    }
    if selector & 2 != 0 {
        let mut decoder = MessageDecoder::default();
        let result = if selector & 1 == 0 {
            decoder.decode(body)
        } else {
            decoder.decode_embedded(body)
        };
        if let Ok(MessageDisposition::New(envelope)) = result {
            let _result = decode_group_reaction(&envelope);
        }
        return;
    }
    let behavior = if selector & 1 == 0 {
        PushBehavior::LegacyVideoAck
    } else {
        PushBehavior::InfoSyncState
    };
    let entry = PushPlanEntry::new(
        "push.input".to_owned(),
        behavior,
        (behavior == PushBehavior::LegacyVideoAck).then(|| "push.response".to_owned()),
        0,
        2 * 1024 * 1024,
    )
    .expect("fixed fuzz plan must be valid");
    let mut processor = PushProcessor::default();
    let mut sync = OnlineSyncState::default();
    let _result = processor.apply(&entry, body, &mut sync, 256, 16 * 1024 * 1024);
});
