#![no_main]
use libfuzzer_sys::fuzz_target;
use wove::{input::Decoder, Event};

fuzz_target!(|data: &[u8]| {
    let mut whole = Decoder::default();
    let mut expected = whole.push(data);
    expected.extend(whole.flush_escape());
    let mut fragmented = Decoder::default();
    let mut actual = Vec::new();
    let chunk = usize::from(data.first().copied().unwrap_or(0)) % 32 + 1;
    for bytes in data.chunks(chunk) {
        actual.extend(fragmented.push(bytes));
    }
    actual.extend(fragmented.flush_escape());
    assert_eq!(actual, expected);
    for event in actual {
        if let Event::Paste(text) = event {
            // Invalid UTF-8 can expand each retained input byte to a three-byte replacement.
            assert!(text.len() <= 3 * 65536);
        }
    }
});
