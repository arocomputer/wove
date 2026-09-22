//! Differential full-screen output to any byte sink.
use super::pen::{Depth, Shadow};
use crate::Buffer;
use std::io::{self, Write};

/// Writes changed cells to any byte sink. Does not acquire terminal modes.
pub struct Renderer {
    shadow: Shadow,
}

impl Default for Renderer {
    fn default() -> Self {
        Self::with_depth(Depth::default())
    }
}

impl Renderer {
    /// Map colors to what the receiving terminal can show.
    pub fn with_depth(depth: Depth) -> Self {
        Self {
            shadow: Shadow::new(depth),
        }
    }

    /// Forget physical contents, for example after an external terminal write.
    pub fn invalidate(&mut self) {
        self.shadow.previous = None;
        // Leaving a session resets the shape, and a failed write may have too.
        self.shadow.forget_cursor();
    }

    /// Emit changed cells as one synchronized update, in a single write, so
    /// nothing else that writes to the terminal can land inside a frame.
    /// A failed write invalidates the shadow frame, so the next call repaints
    /// everything instead of losing changes.
    pub fn draw(&mut self, writer: &mut impl Write, frame: &Buffer) -> io::Result<()> {
        self.render(frame);
        self.shadow.send(writer).inspect_err(|_| self.invalidate())
    }

    /// The bytes that bring the terminal from the last frame to `frame`, as
    /// one synchronized update; empty when nothing differs. Rows the renderer
    /// has no record of are written whole, with trailing blanks erased.
    ///
    /// The renderer assumes the bytes arrive. Send them in one write, and call
    /// `invalidate` if it fails; `draw` does both. The slice is reused by the
    /// next call, so a transport that writes later copies it first.
    pub fn render(&mut self, frame: &Buffer) -> &[u8] {
        let shadow = &mut self.shadow;
        if shadow.drawn(frame) {
            shadow.output.clear();
            return &shadow.output;
        }
        let previous = shadow.previous.take();
        let old = previous.as_ref().filter(|old| old.area() == frame.area());
        let cursor_changed = shadow.cursor_changed(frame, old);
        let mut pen = shadow.begin();
        let output = &mut shadow.output;
        let empty = output.len();
        let mut next_position = None;
        for y in 0..frame.area().height {
            let Some(old) = old else {
                let _ = write!(output, "\x1b[{};1H", y + 1);
                pen.row(output, frame, frame.row(y));
                continue;
            };
            if frame.same_row(old, y) {
                continue;
            }
            let before = old.row(y);
            for (x, cell) in frame.row(y).iter().enumerate() {
                if cell.width == 0 || frame.same_cell(cell, old, &before[x]) {
                    continue;
                }
                let x = x as u16;
                if next_position != Some((x, y)) {
                    let _ = write!(output, "\x1b[{};{}H", y + 1, x + 1);
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
        if output.len() == empty && !cursor_changed {
            // The shadow frame is only copied when something was written.
            shadow.keep(frame, previous);
        } else {
            shadow.finish(pen, frame, previous, frame.cursor());
        }
        &shadow.output
    }

    /// Whether `frame` is the one last rendered, so `render` would return no
    /// bytes without comparing a cell.
    #[cfg_attr(not(feature = "terminal"), allow(dead_code))]
    pub(crate) fn current(&self, frame: &Buffer) -> bool {
        self.shadow.drawn(frame)
    }
}
