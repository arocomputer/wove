//! Inline output: frames grow downward on the main screen and their head
//! scrolls into the terminal's own history.
//!
//! A frame is a logical column of rows that may be taller than the screen.
//! `top + row` maps a frame row onto a screen row; `top` goes negative as the
//! frame outgrows the screen. Rows above the screen belong to the terminal and
//! are never rewritten, so only the reachable suffix of a frame is compared.
//!
//! Growth that fits is scrolled in with newlines and then diffed row by row.
//! Growth whose first changed row would itself scroll away is printed in order
//! from that row, so every row reaches history with its final content.
//!
//! `commit` releases finished rows: they stay where they are and later frames
//! start beneath them, which keeps frames small however long the session runs.
//!
//! Absolute positioning starts with a carriage return, so it never depends on
//! the terminal's pending-wrap state.
use super::pen::{trimmed, Depth, Pen};
use super::Slot;
use crate::Buffer;
use std::io::{self, Write};

/// Writes inline frames to any byte sink. Does not acquire terminal modes.
pub struct Inline {
    /// Screen row of frame row zero; negative once the head has scrolled away.
    top: i64,
    /// The last frame drawn, minus any committed rows.
    previous: Option<Buffer>,
    /// Screen dimensions at the last draw. A change invalidates every position.
    screen: Option<(u16, u16)>,
    /// The version of the last frame drawn; zero when unknown.
    seen: u64,
    /// Positions are unknown; the next draw clears the visible screen first.
    redraw: bool,
    output: Vec<u8>,
    depth: Depth,
}

impl Inline {
    /// Start at `anchor`, the zero-based screen row of the launch cursor.
    pub fn new(anchor: u16, depth: Depth) -> Self {
        Self {
            top: i64::from(anchor),
            previous: None,
            screen: None,
            seen: 0,
            redraw: false,
            output: Vec::new(),
            depth,
        }
    }

    /// Forget physical positions after a resize, a failed write, or another
    /// writer. The next draw clears the visible screen, never the scrollback,
    /// and repaints the frame's tail from the top. Committed rows that were
    /// still visible are gone; an application that wants them back draws them
    /// again and commits them again.
    pub fn invalidate(&mut self) {
        self.previous = None;
        self.redraw = true;
    }

    /// Release the first `rows` rows of the last frame. They are never drawn
    /// again, and the next frame's first row sits directly beneath them.
    pub fn commit(&mut self, rows: u16) {
        let Some(previous) = &mut self.previous else {
            return;
        };
        let rows = rows.min(previous.area().height);
        *previous = previous.tail(rows);
        self.seen = 0;
        self.top += i64::from(rows);
    }

    /// Draw a frame as wide as the screen and of any height, as one
    /// synchronized update. `height` is the screen height in rows.
    pub fn draw(&mut self, writer: &mut impl Write, frame: &Buffer, height: u16) -> io::Result<()> {
        let rows = i64::from(height.max(1));
        let len = i64::from(frame.area().height);
        let screen = Some((frame.area().width, height));
        if frame.version != 0 && frame.version == self.seen && !self.redraw && self.screen == screen
        {
            return Ok(());
        }
        self.seen = 0;
        let redraw = self.redraw || self.screen.is_some_and(|old| Some(old) != screen);
        if self.screen.is_none() {
            self.top = self.top.min(rows - 1);
        }
        self.screen = screen;
        let mut previous = self.previous.take().filter(|_| !redraw);
        let old_len = previous
            .as_ref()
            .map_or(0, |old| i64::from(old.area().height));
        let same = |old: &Option<Buffer>, index: i64| {
            old.as_ref()
                .is_some_and(|old| frame.same_row(old, index as u16))
        };
        let reachable = (-self.top).max(0);
        let mut first_changed =
            (reachable..len.max(old_len)).find(|&index| !same(&previous, index));
        let cursor_moved = previous
            .as_ref()
            .is_none_or(|old| old.cursor() != frame.cursor());
        if first_changed.is_none() && !cursor_moved && !redraw {
            self.previous = previous;
            self.seen = frame.version;
            return Ok(());
        }
        // A failed write leaves positions unknown until a draw completes.
        self.redraw = true;

        let output = &mut self.output;
        output.clear();
        let mut pen = Pen::new(self.depth);
        // A full row leaves the cursor on its last cell, where erasing to the
        // end of the line would eat that cell. Full rows need no erase.
        let mut put = |output: &mut Vec<u8>, cells: &[Slot]| {
            let used = trimmed(cells);
            for cell in used.iter().filter(|cell| cell.width > 0) {
                pen.cell(output, frame, cell);
            }
            pen.reset(output);
            if used.len() < cells.len() || cells.is_empty() {
                output.extend_from_slice(b"\x1b[K");
            }
        };

        // Screen rows that may hold stale content from the last frame.
        let mut stale = self.top + old_len;
        if !redraw && len < old_len && (self.top + len <= 0 || len >= rows) {
            // A shrinking frame that fills the screen keeps its last row at the
            // bottom. Every visible row moves, so all of them are rewritten.
            self.top = (rows - len).min(0);
            previous = None;
            first_changed = Some(-self.top);
            stale = rows;
        }
        let top = self.top.min(rows - len);
        let scroll = self.top - top;
        if redraw {
            output.extend_from_slice(b"\r\x1b[H\x1b[2J");
            self.top = (rows - len).min(0);
            for (i, index) in (-self.top..len).enumerate() {
                if i > 0 {
                    output.extend_from_slice(b"\r\n");
                }
                put(output, frame.row(index as u16));
            }
        } else if let Some(first) = first_changed.filter(|first| scroll > 0 && top + first < 0) {
            // Flow: print from the first changed row and let the terminal
            // scroll. A row one past the bottom scrolls in with one newline.
            let position = self.top + first;
            if position >= rows {
                write!(output, "\r\x1b[{rows};1H\n")?;
            } else {
                write!(output, "\r\x1b[{};1H", position + 1)?;
            }
            for (i, index) in (first..len).enumerate() {
                if i > 0 {
                    output.extend_from_slice(b"\r\n");
                }
                put(output, frame.row(index as u16));
            }
            self.top = top;
        } else {
            if scroll > 0 {
                write!(output, "\r\x1b[{rows};1H")?;
                output.extend(std::iter::repeat_n(b'\n', scroll as usize));
            }
            self.top = top;
            stale -= scroll;
            let mut written = None;
            for screen_row in self.top.max(0)..rows.min(stale.max(self.top + len)) {
                let index = screen_row - self.top;
                if index < old_len && same(&previous, index) {
                    continue;
                }
                if written == Some(screen_row - 1) {
                    output.extend_from_slice(b"\r\n");
                } else {
                    write!(output, "\r\x1b[{};1H", screen_row + 1)?;
                }
                put(output, frame.row(index.min(i64::from(u16::MAX)) as u16));
                written = Some(screen_row);
            }
        }

        match frame.cursor().map(|(x, y)| (x, self.top + i64::from(y))) {
            Some((x, y)) if (0..rows).contains(&y) => {
                write!(output, "\r\x1b[{};{}H\x1b[?25h", y + 1, x + 1)?
            }
            _ => {}
        }
        writer.write_all(b"\x1b[?2026h\x1b[?25l")?;
        writer.write_all(output)?;
        writer.write_all(b"\x1b[?2026l")?;
        writer.flush()?;
        self.redraw = false;
        self.seen = frame.version;
        match &mut previous {
            Some(buffer) => buffer.clone_from(frame),
            None => previous = Some(frame.clone()),
        }
        self.previous = previous;
        Ok(())
    }

    /// Park the cursor on a fresh line beneath the last frame, where a shell
    /// prompt or a child program can continue.
    pub fn finish(&mut self, writer: &mut impl Write) -> io::Result<()> {
        let len = self
            .previous
            .as_ref()
            .map_or(0, |old| i64::from(old.area().height));
        let rows = i64::from(self.screen.map_or(1, |(_, rows)| rows.max(1)));
        if len == 0 {
            write!(writer, "\r\x1b[{};1H", (self.top + 1).clamp(1, rows))?;
        } else {
            write!(writer, "\r\x1b[{};1H\r\n", (self.top + len).clamp(1, rows))?;
        }
        writer.flush()
    }
}
