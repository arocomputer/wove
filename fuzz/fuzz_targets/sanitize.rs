#![no_main]
use libfuzzer_sys::fuzz_target;
use wove::{Buffer, Style};

fuzz_target!(|data: &[u8]| {
    let text = String::from_utf8_lossy(data);
    let mut frame = Buffer::new(32, 4);
    frame.write(frame.area(), &text, Style::default());
    for y in 0..4 {
        for x in 0..32 {
            let cell = frame.cell(x, y).unwrap();
            assert!(!cell.symbol().chars().any(char::is_control));
        }
    }
});
