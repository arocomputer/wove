//! Headless screens, clocks, frame recording, and plain-text frame snapshots.
//!
//! `Screen` drives the same element tree and input dispatch used by an
//! interactive terminal. `snapshot` renders a frame as text that `./x ui`
//! also writes for real terminals, and `assert_frame` compares it with an
//! expected snapshot, reporting the first rows that differ.
use crate::{Buffer, Dispatch, Error, Event, Tree};

pub struct Screen {
    pub tree: Tree,
    width: u16,
    height: u16,
}
impl Screen {
    pub fn new(width: u16, height: u16) -> Self {
        Self {
            tree: Tree::new(),
            width,
            height,
        }
    }
    pub fn resize(&mut self, width: u16, height: u16) {
        self.width = width;
        self.height = height;
    }
    pub fn frame(&mut self) -> Result<&Buffer, Error> {
        self.tree.frame(self.width, self.height)
    }
    pub fn send(&mut self, event: impl Into<Event>) -> Result<Dispatch, Error> {
        self.frame()?;
        self.tree.dispatch(event.into())
    }
    /// Press and release the left button on a cell.
    pub fn click(&mut self, x: u16, y: u16) -> Result<Dispatch, Error> {
        use crate::{Button, Mouse, MouseKind};
        let press = self.send(Event::Mouse(Mouse::new(
            x,
            y,
            MouseKind::Down(Button::Left),
        )))?;
        self.send(Event::Mouse(Mouse::new(x, y, MouseKind::Up(Button::Left))))?;
        Ok(press)
    }
    /// Press on one cell, drag to another, and release there.
    pub fn drag(&mut self, from: (u16, u16), to: (u16, u16)) -> Result<Dispatch, Error> {
        use crate::{Button, Mouse, MouseKind};
        let at = |(x, y), kind| Event::Mouse(Mouse::new(x, y, kind));
        self.send(at(from, MouseKind::Down(Button::Left)))?;
        let dragged = self.send(at(to, MouseKind::Drag(Button::Left)))?;
        self.send(at(to, MouseKind::Up(Button::Left)))?;
        Ok(dragged)
    }
}

/// A clock advanced explicitly by tests, with no sleeping or wall-clock reads.
#[derive(Default)]
pub struct Clock {
    now: std::time::Duration,
}
impl Clock {
    pub fn now(&self) -> std::time::Duration {
        self.now
    }
    pub fn advance(&mut self, elapsed: std::time::Duration) {
        self.now = self.now.saturating_add(elapsed);
    }
}

/// Retains only changed frames, including their styles and cursor positions.
#[derive(Default)]
pub struct Recorder {
    frames: Vec<(std::time::Duration, Buffer)>,
}
impl Recorder {
    pub fn record(&mut self, at: std::time::Duration, frame: &Buffer) {
        if self
            .frames
            .last()
            .is_none_or(|(_, previous)| previous != frame)
        {
            self.frames.push((at, frame.clone()));
        }
    }
    pub fn frames(&self) -> &[(std::time::Duration, Buffer)] {
        &self.frames
    }
    pub fn clear(&mut self) {
        self.frames.clear();
    }
}

/// A frame as plain text for comparison with an expected snapshot.
///
/// Each row appears once with trailing blanks removed; blank rows at the
/// bottom are dropped. A wide grapheme is written once, as a terminal shows
/// it. The last line is `@cursor x,y` when the frame shows a cursor and
/// `@cursor none` otherwise. Every line ends with a newline. Styles are not
/// included; inspect `Buffer::cell` to pin them.
pub fn snapshot(frame: &Buffer) -> String {
    let mut rows: Vec<String> = frame
        .lines()
        .into_iter()
        .map(|row| row.trim_end().to_owned())
        .collect();
    while rows.last().is_some_and(String::is_empty) {
        rows.pop();
    }
    let mut text = String::new();
    for row in rows {
        text.push_str(&row);
        text.push('\n');
    }
    match frame.cursor() {
        Some((x, y)) => text.push_str(&format!("@cursor {x},{y}\n")),
        None => text.push_str("@cursor none\n"),
    }
    text
}

/// Describe where two snapshots first differ, or `None` when they are equal.
///
/// The report shows up to two matching lines before the difference, then up to
/// three expected lines marked `-` and actual lines marked `+`, each with its
/// line number and a `|` that shows where the text starts. A side that has
/// already ended, as when one snapshot lacks its final newline, shows `(end)`.
pub fn diff(expected: &str, actual: &str) -> Option<String> {
    if expected == actual {
        return None;
    }
    let old: Vec<&str> = expected.split('\n').collect();
    let new: Vec<&str> = actual.split('\n').collect();
    let first = old
        .iter()
        .zip(&new)
        .position(|(a, b)| a != b)
        .unwrap_or(old.len().min(new.len()));
    let mut report = format!("frames differ at line {}\n", first + 1);
    let mut show = |mark: char, lines: &[&str], range: std::ops::Range<usize>| {
        if range.start >= lines.len() && !range.is_empty() {
            report.push_str(&format!("{mark}     (end)\n"));
        }
        for (index, line) in lines.iter().enumerate().take(range.end).skip(range.start) {
            report.push_str(&format!("{mark}{:>4} |{line}\n", index + 1));
        }
    };
    show(' ', &old, first.saturating_sub(2)..first);
    show('-', &old, first..first + 3);
    show('+', &new, first..first + 3);
    Some(report)
}

/// Panic with a `diff` report unless the frame's `snapshot` equals `expected`.
#[track_caller]
pub fn assert_frame(frame: &Buffer, expected: &str) {
    if let Some(report) = diff(expected, &snapshot(frame)) {
        panic!("{report}");
    }
}
