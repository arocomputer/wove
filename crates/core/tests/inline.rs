//! Inline rendering checked against what a terminal would show and keep in
//! its scrollback, not against escape bytes.
use wove::{Buffer, Depth, Inline, Style};

/// Just enough of a terminal for the sequences the renderer writes: absolute
/// positioning, carriage return, line feed with scrolling, and erasing.
struct Vt {
    width: usize,
    rows: Vec<Vec<char>>,
    scrollback: Vec<String>,
    cursor: (usize, usize),
    bytes: Vec<u8>,
}
impl Vt {
    fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            rows: vec![vec![' '; width]; height],
            scrollback: Vec::new(),
            cursor: (0, 0),
            bytes: Vec::new(),
        }
    }
    fn screen(&self) -> Vec<String> {
        let line = |row: &Vec<char>| row.iter().collect::<String>().trim_end().to_owned();
        self.rows.iter().map(line).collect()
    }
    /// Everything the user can scroll back through, then the screen.
    fn history(&self) -> Vec<String> {
        let mut all = self.scrollback.clone();
        all.extend(self.screen());
        while all.last().is_some_and(String::is_empty) {
            all.pop();
        }
        all
    }
    /// Change the width as a terminal that does not reflow would: rows are
    /// cut or padded where they stand.
    fn set_width(&mut self, width: usize) {
        self.width = width;
        for row in &mut self.rows {
            row.resize(width, ' ');
        }
    }
    fn feed(&mut self, bytes: &[u8]) {
        self.bytes.extend_from_slice(bytes);
        let text = String::from_utf8(bytes.to_vec()).unwrap();
        let mut chars = text.chars().peekable();
        while let Some(c) = chars.next() {
            match c {
                '\r' => self.cursor.0 = 0,
                '\n' if self.cursor.1 + 1 == self.rows.len() => {
                    let gone = self.rows.remove(0);
                    self.scrollback
                        .push(gone.iter().collect::<String>().trim_end().to_owned());
                    self.rows.push(vec![' '; self.width]);
                }
                '\n' => self.cursor.1 += 1,
                '\x1b' if chars.next_if_eq(&']').is_some() => {
                    while chars.next().is_some_and(|c| c != '\\') {}
                }
                '\x1b' => {
                    assert_eq!(chars.next(), Some('['));
                    let mut body = String::new();
                    let end = loop {
                        match chars.next().unwrap() {
                            c @ ('@'..='~') => break c,
                            c => body.push(c),
                        }
                    };
                    match (end, body.as_str()) {
                        ('H', "") => self.cursor = (0, 0),
                        ('H', at) => {
                            let (row, column) = at.split_once(';').unwrap();
                            self.cursor = (
                                column.parse::<usize>().unwrap() - 1,
                                row.parse::<usize>().unwrap() - 1,
                            );
                        }
                        // Clearing would flash, and some terminals push the
                        // cleared screen into scrollback.
                        ('J', other) => panic!("rows are repainted, never cleared: ESC[{other}J"),
                        ('K', _) => self.rows[self.cursor.1][self.cursor.0..].fill(' '),
                        _ => {}
                    }
                }
                c => {
                    self.rows[self.cursor.1][self.cursor.0] = c;
                    self.cursor.0 = (self.cursor.0 + 1).min(self.width - 1);
                }
            }
        }
    }
}

fn frame(width: usize, rows: &[&str]) -> Buffer {
    let mut buffer = Buffer::new(width as u16, rows.len() as u16);
    for (y, row) in rows.iter().enumerate() {
        let area = wove::Rect::new(0, y as u16, width as u16, 1);
        buffer.write(area, row, Style::default());
    }
    buffer
}

/// Draw a frame as wide as `vt` and feed the output to it, returning whether
/// anything was written.
fn draw(inline: &mut Inline, vt: &mut Vt, rows: &[&str]) -> bool {
    let mut bytes = Vec::new();
    let wrote = inline
        .draw(&mut bytes, &frame(vt.width, rows), vt.rows.len() as u16)
        .unwrap();
    assert_eq!(wrote, !bytes.is_empty());
    vt.feed(&bytes);
    wrote
}

#[test]
fn a_frame_starts_at_the_launch_row_and_leaves_the_shell_above_it() {
    let mut vt = Vt::new(8, 4);
    vt.feed(b"$ app\r\n");
    let mut inline = Inline::new(1, Depth::Rgb);
    draw(&mut inline, &mut vt, &["one", "two"]);
    assert_eq!(vt.screen(), ["$ app", "one", "two", ""]);
    let before = vt.bytes.len();
    assert!(!draw(&mut inline, &mut vt, &["one", "two"]));
    assert_eq!(vt.bytes.len(), before, "an unchanged frame writes nothing");
}

#[test]
fn growth_past_the_bottom_reaches_scrollback_in_order_with_final_content() {
    let mut vt = Vt::new(8, 3);
    let mut inline = Inline::new(0, Depth::Rgb);
    draw(&mut inline, &mut vt, &["a", "b"]);
    // Row b changes in the same frame that pushes it off the screen.
    draw(&mut inline, &mut vt, &["a", "B", "c", "d", "e", "f"]);
    assert_eq!(vt.history(), ["a", "B", "c", "d", "e", "f"]);
    assert_eq!(vt.screen(), ["d", "e", "f"]);
    // Rows that left the screen belong to the terminal and are never rewritten.
    draw(&mut inline, &mut vt, &["A", "B", "c", "d", "e", "F"]);
    assert_eq!(vt.history(), ["a", "B", "c", "d", "e", "F"]);
}

#[test]
fn small_growth_scrolls_and_rewrites_only_what_changed() {
    let mut vt = Vt::new(8, 3);
    let mut inline = Inline::new(0, Depth::Rgb);
    draw(&mut inline, &mut vt, &["a", "b", "c"]);
    draw(&mut inline, &mut vt, &["a", "b", "c", "d"]);
    assert_eq!(vt.history(), ["a", "b", "c", "d"]);
}

#[test]
fn committed_rows_stay_put_while_later_frames_start_beneath_them() {
    let mut vt = Vt::new(8, 4);
    let mut inline = Inline::new(0, Depth::Rgb);
    draw(&mut inline, &mut vt, &["done", "done 2", "live"]);
    inline.commit(2);
    draw(&mut inline, &mut vt, &["live!", "dock"]);
    assert_eq!(vt.screen(), ["done", "done 2", "live!", "dock"]);
    draw(&mut inline, &mut vt, &["dock"]);
    assert_eq!(vt.screen(), ["done", "done 2", "dock", ""]);
}

#[test]
fn a_shrinking_full_screen_frame_keeps_its_last_row_at_the_bottom() {
    let mut vt = Vt::new(8, 3);
    let mut inline = Inline::new(0, Depth::Rgb);
    draw(&mut inline, &mut vt, &["a", "b", "c", "d", "dock"]);
    draw(&mut inline, &mut vt, &["a", "b", "c", "dock"]);
    assert_eq!(vt.screen(), ["b", "c", "dock"]);
}

#[test]
fn a_resize_repaints_the_visible_tail_without_touching_scrollback() {
    let mut vt = Vt::new(8, 3);
    let mut inline = Inline::new(0, Depth::Rgb);
    draw(&mut inline, &mut vt, &["a", "b", "c", "d"]);
    vt.rows.pop();
    draw(&mut inline, &mut vt, &["a", "b", "c", "d"]);
    assert_eq!(vt.scrollback, ["a"]);
    assert_eq!(vt.screen(), ["c", "d"]);

    // A frame that fits moves to the top, and rows beneath it are erased.
    let mut vt = Vt::new(8, 4);
    vt.feed(b"$ app\r\n");
    let mut inline = Inline::new(1, Depth::Rgb);
    draw(&mut inline, &mut vt, &["a", "b"]);
    vt.rows.pop();
    draw(&mut inline, &mut vt, &["a", "b"]);
    assert_eq!(vt.screen(), ["a", "b", ""]);
}

#[test]
fn committed_rows_still_on_screen_are_repainted_after_a_resize() {
    let mut vt = Vt::new(8, 6);
    let mut inline = Inline::new(0, Depth::Rgb);
    draw(&mut inline, &mut vt, &["a", "b", "c"]);
    inline.commit(2);
    draw(&mut inline, &mut vt, &["c", "d"]);
    assert_eq!(vt.screen(), ["a", "b", "c", "d", "", ""]);
    vt.rows.pop();
    draw(&mut inline, &mut vt, &["c", "d", "e"]);
    assert_eq!(vt.screen(), ["a", "b", "c", "d", "e"]);
    assert!(vt.scrollback.is_empty());
}

#[test]
fn committed_rows_that_scrolled_away_are_left_to_the_scrollback() {
    let mut vt = Vt::new(8, 4);
    let mut inline = Inline::new(0, Depth::Rgb);
    draw(&mut inline, &mut vt, &["a", "b"]);
    inline.commit(2);
    draw(&mut inline, &mut vt, &["c", "d", "e"]);
    assert_eq!(vt.history(), ["a", "b", "c", "d", "e"]);
    draw(&mut inline, &mut vt, &["c"]);
    vt.rows.pop();
    draw(&mut inline, &mut vt, &["c"]);
    assert_eq!(vt.screen(), ["b", "c", ""]);
    assert_eq!(vt.scrollback, ["a"]);
}

#[test]
fn committed_rows_are_cropped_to_a_narrower_screen() {
    let mut vt = Vt::new(8, 3);
    let mut inline = Inline::new(0, Depth::Rgb);
    draw(&mut inline, &mut vt, &["abcdefgh", "x"]);
    inline.commit(1);
    vt.set_width(4);
    draw(&mut inline, &mut vt, &["x"]);
    assert_eq!(vt.screen(), ["abcd", "x", ""]);
}

#[test]
fn committing_rows_already_in_scrollback_does_not_restore_them_on_resize() {
    let mut vt = Vt::new(8, 3);
    let mut inline = Inline::new(0, Depth::Rgb);
    draw(&mut inline, &mut vt, &["a", "b", "c", "d", "e"]);
    inline.commit(3);
    vt.rows.resize(5, vec![' '; vt.width]);
    draw(&mut inline, &mut vt, &["d", "e"]);
    assert_eq!(vt.scrollback, ["a", "b"]);
    assert_eq!(vt.screen(), ["c", "d", "e", "", ""]);
}

#[test]
fn an_invalidated_frame_still_parks_commits_and_maps_rows_until_redrawn() {
    let mut vt = Vt::new(8, 4);
    vt.feed(b"$ app\r\n");
    let mut inline = Inline::new(1, Depth::Rgb);
    draw(&mut inline, &mut vt, &["a", "b", "c"]);
    inline.invalidate();
    assert_eq!(inline.frame_row(2), Some(1));
    inline.commit(1);
    assert_eq!(inline.frame_row(2), Some(0));
    let mut bytes = Vec::new();
    inline.finish(&mut bytes).unwrap();
    vt.feed(&bytes);
    vt.feed(b"$ ");
    assert_eq!(vt.history(), ["$ app", "a", "b", "c", "$"]);
}

#[test]
fn a_screen_row_maps_onto_the_frame_wherever_it_has_scrolled_to() {
    let mut vt = Vt::new(8, 3);
    vt.feed(b"$ app\r\n");
    let mut inline = Inline::new(1, Depth::Rgb);
    draw(&mut inline, &mut vt, &["a", "b"]);
    assert_eq!(
        inline.frame_row(0),
        None,
        "the shell's row is not the frame's"
    );
    assert_eq!(inline.frame_row(1), Some(0));
    draw(&mut inline, &mut vt, &["a", "b", "c", "d"]);
    assert_eq!(vt.screen(), ["b", "c", "d"]);
    assert_eq!(inline.frame_row(0), Some(1));
}

#[test]
fn finishing_parks_the_cursor_on_a_fresh_line_below_the_frame() {
    let mut vt = Vt::new(8, 3);
    let mut inline = Inline::new(0, Depth::Rgb);
    draw(&mut inline, &mut vt, &["a", "b", "c"]);
    let mut bytes = Vec::new();
    inline.finish(&mut bytes).unwrap();
    vt.feed(&bytes);
    vt.feed(b"$ ");
    assert_eq!(vt.history(), ["a", "b", "c", "$"]);
}

#[test]
fn finishing_after_committing_every_row_preserves_the_bottom_row() {
    for height in [2, 3, 4] {
        let mut vt = Vt::new(8, height);
        let mut inline = Inline::new(0, Depth::Rgb);
        draw(&mut inline, &mut vt, &["a", "b", "done"]);
        inline.commit(3);
        let mut bytes = Vec::new();
        inline.finish(&mut bytes).unwrap();
        vt.feed(&bytes);
        vt.feed(b"$ ");
        assert_eq!(vt.history(), ["a", "b", "done", "$"]);
    }
}

#[test]
fn finishing_without_drawing_keeps_the_launch_cursor() {
    let mut vt = Vt::new(8, 3);
    vt.feed(b"$ app\r\n");
    let mut inline = Inline::new(1, Depth::Rgb);
    let mut bytes = Vec::new();
    inline.finish(&mut bytes).unwrap();
    vt.feed(&bytes);
    vt.feed(b"$ ");
    assert_eq!(vt.history(), ["$ app", "$"]);
}
