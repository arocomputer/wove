//! Reproducible CPU rendering workload; no terminal or network is required.
use std::{hint::black_box, time::Instant};
use weft::{Buffer, Style, Text, Widget};

fn main() {
    if std::env::args().any(|arg| arg == "--startup") {
        return;
    }
    let start = Instant::now();
    let mut buffer = Buffer::new(120, 40);
    for _ in 0..10_000 {
        buffer.clear();
        Text {
            content: "weft benchmark\nUnicode: café 日本語 👩‍💻\nAn application-owned frame.",
            style: Style::default(),
        }
        .render(buffer.area(), &mut buffer);
        black_box(&buffer);
    }
    println!("{:.3} us/frame", start.elapsed().as_secs_f64() * 100.0);
}
