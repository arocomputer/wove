//! Public contracts for clipping, grapheme ownership, composition, and layouts.
use weft::{Axis, Border, Buffer, Constraint, Layout, Rect, Style, Text, Widget};

#[test]
fn clipping_never_splits_a_wide_grapheme() {
    let mut frame = Buffer::new(3, 1);
    assert_eq!(frame.write(frame.area(), "a界b", Style::default()), 3);
    assert_eq!(frame.lines(), ["a界"]);
    frame.clear();
    assert_eq!(
        frame.write(Rect::new(0, 0, 2, 1), "a界", Style::default()),
        1
    );
    assert_eq!(frame.lines(), ["a  "]);
}

#[test]
fn overwriting_a_continuation_clears_the_entire_old_grapheme() {
    let mut frame = Buffer::new(4, 1);
    frame.write(frame.area(), "界!", Style::default());
    frame.write(Rect::new(1, 0, 1, 1), "x", Style::default());
    assert_eq!(frame.lines(), [" x! "]);
}

#[test]
fn combining_and_joined_emoji_are_single_graphemes() {
    let mut frame = Buffer::new(3, 1);
    frame.write(frame.area(), "e\u{301}👩‍💻", Style::default());
    assert_eq!(frame.cell(0, 0).unwrap().symbol(), "e\u{301}");
    assert_eq!(frame.cell(1, 0).unwrap().symbol(), "👩‍💻");
    assert_eq!(frame.cell(2, 0).unwrap().symbol(), "");
}

#[test]
fn content_cannot_emit_terminal_control_bytes() {
    let mut frame = Buffer::new(20, 1);
    frame.write(frame.area(), "a\x1b[2J\r\n\t\u{9b}b", Style::default());
    assert_eq!(frame.lines()[0].trim_end(), "a[2Jb");
}

#[test]
fn fixed_tracks_precede_weighted_allocation_with_exact_rounding() {
    let result = Layout {
        axis: Axis::Horizontal,
        tracks: &[
            Constraint::Fill(1),
            Constraint::Fixed(2),
            Constraint::Fill(2),
        ],
        gap: 1,
    }
    .split(Rect::new(3, 4, 14, 5));
    assert_eq!(
        result,
        [
            Rect::new(3, 4, 3, 5),
            Rect::new(7, 4, 2, 5),
            Rect::new(10, 4, 7, 5)
        ]
    );
}

#[test]
fn tiny_layouts_remain_inside_the_parent() {
    for width in 0..20 {
        let regions = Layout {
            axis: Axis::Horizontal,
            tracks: &[
                Constraint::Fixed(8),
                Constraint::Fill(1),
                Constraint::Fill(2),
            ],
            gap: 3,
        }
        .split(Rect::new(0, 0, width, 1));
        for region in regions {
            assert!(region.x + region.width <= width);
        }
    }
}

#[test]
fn nested_widgets_receive_only_the_border_interior() {
    let mut frame = Buffer::new(8, 4);
    Border {
        child: Text {
            content: "hello world\nnext\nhidden",
            style: Style::default(),
        },
        style: Style::default(),
    }
    .render(frame.area(), &mut frame);
    assert_eq!(
        frame.lines(),
        ["┌──────┐", "│hello │", "│next  │", "└──────┘"]
    );
}

#[cfg(feature = "terminal")]
#[test]
fn identical_frames_emit_nothing_and_failed_writes_repaint() {
    use std::io::{self, Write};
    struct Broken;
    impl Write for Broken {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::Error::other("disconnected"))
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let mut renderer = weft::Renderer::default();
    let mut frame = Buffer::new(3, 1);
    let mut bytes = Vec::new();
    renderer.draw(&mut bytes, &frame).unwrap();
    bytes.clear();
    renderer.draw(&mut bytes, &frame).unwrap();
    assert!(bytes.is_empty());
    frame.write(frame.area(), "new", Style::default());
    assert!(renderer.draw(&mut Broken, &frame).is_err());
    renderer.draw(&mut bytes, &frame).unwrap();
    assert!(String::from_utf8(bytes).unwrap().contains("new"));
}
