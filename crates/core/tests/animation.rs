use std::time::Duration;
use wove::{
    animation::Tween,
    testing::{Clock, Recorder},
    Buffer,
};
#[test]
fn delayed_and_zero_duration_transitions_have_explicit_boundaries() {
    let mut clock = Clock::default();
    let tween = Tween {
        from: 2.0,
        to: 10.0,
        start: Duration::from_secs(1),
        duration: Duration::from_secs(2),
    };
    assert_eq!(tween.value(clock.now()), 2.0);
    clock.advance(Duration::from_secs(2));
    assert_eq!(tween.value(clock.now()), 6.0);
    clock.advance(Duration::from_secs(10));
    assert_eq!(tween.value(clock.now()), 10.0);
    assert!(tween.finished(clock.now()));
    assert_eq!(
        Tween {
            duration: Duration::ZERO,
            ..tween
        }
        .value(Duration::ZERO),
        2.0
    );
}
#[test]
fn recorder_retains_style_changes_but_not_identical_frames() {
    let mut recorder = Recorder::default();
    let mut frame = Buffer::new(1, 1);
    recorder.record(Duration::ZERO, &frame);
    recorder.record(Duration::from_secs(1), &frame);
    frame.write(
        frame.area(),
        " ",
        wove::Style {
            bold: true,
            ..Default::default()
        },
    );
    recorder.record(Duration::from_secs(2), &frame);
    assert_eq!(recorder.frames().len(), 2);
    assert!(recorder.frames()[1].1.cell(0, 0).unwrap().style().bold);
}
