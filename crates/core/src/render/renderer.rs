//! Differential full-screen output to any byte sink.
use super::pen::{Depth, Pen};
use crate::{Buffer, CursorShape};
use std::io::{self, Write};

/// Writes changed cells to any byte sink. Does not acquire terminal modes.
#[derive(Default)]
pub struct Renderer {
    previous: Option<Buffer>,
    /// The version of the last frame drawn; zero when unknown.
    seen: u64,
    shape: CursorShape,
    output: Vec<u8>,
    depth: Depth,
}

impl Renderer {
    /// Map colors to what the receiving terminal can show.
    pub fn with_depth(depth: Depth) -> Self {
        Self {
            depth,
            ..Self::default()
        }
    }

    /// Forget physical contents, for example after an external terminal write.
    pub fn invalidate(&mut self) {
        self.previous = None;
        self.seen = 0;
    }

    /// Emit changed cells as one synchronized update and flush once. Failed
    /// writes invalidate the shadow frame so a subsequent call repaints
    /// everything instead of losing changes.
    pub fn draw(&mut self, writer: &mut impl Write, frame: &Buffer) -> io::Result<()> {
        if frame.version != 0 && frame.version == self.seen {
            return Ok(());
        }
        self.seen = 0;
        let mut previous = self.previous.take();
        let old = previous.as_ref().filter(|old| old.area() == frame.area());
        let output = &mut self.output;
        output.clear();
        let mut pen = Pen::new(self.depth);
        let mut next_position = None;
        for y in 0..frame.area().height {
            if old.is_some_and(|old| frame.same_row(old, y)) {
                continue;
            }
            let before = old.map(|old| (old, old.row(y)));
            for (x, cell) in frame.row(y).iter().enumerate() {
                let same = |(old, row): (&Buffer, &[_])| frame.same_cell(cell, old, &row[x]);
                if cell.width == 0 || before.is_some_and(same) {
                    continue;
                }
                let x = x as u16;
                if next_position != Some((x, y)) {
                    write!(output, "\x1b[{};{}H", y + 1, x + 1)?;
                }
                pen.cell(output, frame, cell);
                // Terminals disagree on the width of emoji and other clusters,
                // tmux most of all. Only after plain ASCII is the cursor known to
                // be where the next cell goes; otherwise it is placed again.
                next_position = x
                    .checked_add(u16::from(cell.width))
                    .filter(|_| cell.is_ascii())
                    .map(|x| (x, y));
            }
        }
        let cursor_moved = old.is_none_or(|old| old.cursor() != frame.cursor());
        if !output.is_empty() || cursor_moved || frame.cursor_shape() != self.shape {
            pen.reset(output);
            if frame.cursor_shape() != self.shape {
                self.shape = frame.cursor_shape();
                write!(output, "\x1b[{} q", self.shape.code())?;
            }
            match frame.cursor() {
                Some((x, y)) => write!(output, "\x1b[{};{}H\x1b[?25h", y + 1, x + 1)?,
                None => output.extend_from_slice(b"\x1b[?25l"),
            }
            writer.write_all(b"\x1b[?2026h")?;
            writer.write_all(output)?;
            writer.write_all(b"\x1b[?2026l")?;
            writer.flush()?;
            // The shadow frame is only copied when something was written.
            match &mut previous {
                Some(buffer) => buffer.clone_from(frame),
                None => previous = Some(frame.clone()),
            }
        }
        self.previous = previous;
        self.seen = frame.version;
        Ok(())
    }
}
