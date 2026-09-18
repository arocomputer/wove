//! Local frame timings, excluding terminal output. No pass/fail thresholds.
use std::{hint::black_box, time::Instant};
use wove::{elements::Text, Tree};
fn main() {
    let mut tree = Tree::new();
    let text = tree.add(tree.root(), Text::new("wove")).unwrap();
    tree.frame(100, 30).unwrap();
    let start = Instant::now();
    for i in 0..1000 {
        tree.update::<Text>(text, |w| w.content = format!("Frame {i}\nUnicode: 界 👩‍💻"))
            .unwrap();
        black_box(tree.frame(100, 30).unwrap());
    }
    let changed = start.elapsed().as_secs_f64() * 1000.0;
    let start = Instant::now();
    for _ in 0..100_000 {
        black_box(tree.frame(100, 30).unwrap());
    }
    let idle = start.elapsed().as_secs_f64() * 10.0;
    println!("changed frame: {changed:.3} µs\ncached frame: {idle:.6} µs");
}
