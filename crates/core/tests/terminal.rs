#![cfg(feature = "terminal")]
use std::io::{self, Write};
use weft_core::{terminal::Renderer, Buffer, Rect, Style};
#[test]
fn unchanged_frames_emit_nothing_and_failed_output_forces_a_full_repaint() {
    struct Broken;
    impl Write for Broken {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::Error::other("disconnected"))
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let mut renderer = Renderer::default();
    let mut frame = Buffer::new(8, 1);
    frame.write(frame.area(), "hello", Style::default());
    let mut output = Vec::new();
    renderer.draw(&mut output, &frame).unwrap();
    assert!(!output.is_empty());
    output.clear();
    renderer.draw(&mut output, &frame).unwrap();
    assert!(output.is_empty());
    frame.write(Rect::new(0, 0, 1, 1), "H", Style::default());
    assert!(renderer.draw(&mut Broken, &frame).is_err());
    renderer.draw(&mut output, &frame).unwrap();
    assert!(String::from_utf8(output).unwrap().contains("Hello"));
}
