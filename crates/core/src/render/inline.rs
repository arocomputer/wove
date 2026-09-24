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
//! The released rows still on screen are kept until they scroll past the top,
//! so a repaint after a resize puts them back above the frame.
//!
//! Absolute positioning starts with a carriage return, so it never depends on
//! the terminal's pending-wrap state.
use super::buffer::Slot;
use super::pen::{Depth, Pen, Shadow};
use crate::Buffer;
use std::collections::VecDeque;
use std::io::{self, Write};

/// Writes inline frames to any byte sink. Does not acquire terminal modes.
pub struct Inline {
    /// Screen row of frame row zero; negative once the head has scrolled away.
    top: i64,
    /// Screen dimensions at the last draw. A change invalidates every position.
    screen: Option<(u16, u16)>,
    /// Positions are unknown; the next draw repaints every visible row.
    redraw: bool,
    /// Committed rows still on screen, oldest first, directly above the
    /// frame: they occupy the screen rows just above `top`.
    kept: VecDeque<Buffer>,
    /// The last frame drawn, minus any committed rows, and the cursor the
    /// terminal was last told to show.
    shadow: Shadow,
}

impl Inline {
    /// Start at `anchor`, the zero-based screen row of the launch cursor.
    pub fn new(anchor: u16, depth: Depth) -> Self {
        Self {
            top: i64::from(anchor),
            screen: None,
            redraw: false,
            kept: VecDeque::new(),
            shadow: Shadow::new(depth),
        }
    }

    /// Forget physical positions after a resize, a failed write, or another
    /// writer. The next draw repaints from the top of the screen: the
    /// committed rows that were still on screen, as many as fit above the
    /// frame, then the frame's tail, and erases every visible row beneath it.
    /// It never writes to the scrollback. Until then, `commit`, `park`, and
    /// `frame_row` answer from where the last frame was drawn.
    pub fn invalidate(&mut self) {
        self.redraw = true;
        self.shadow.forget_cursor();
    }

    /// The terminal's cursor may have been hidden or reshaped by another
    /// writer that left the rows alone, as a full-screen view on the
    /// alternate screen does. The next draw shows it again.
    #[cfg_attr(not(feature = "terminal"), allow(dead_code))]
    pub(crate) fn forget_cursor(&mut self) {
        self.shadow.forget_cursor();
    }

    /// Release the first `rows` rows of the last frame. They stay where they
    /// are, and the next frame's first row sits directly beneath them. They
    /// are drawn again only by a repaint, while they are still on screen.
    pub fn commit(&mut self, rows: u16) {
        let Some(previous) = &mut self.shadow.previous else {
            return;
        };
        let rows = rows.min(previous.area().height);
        if rows > 0 {
            self.kept.push_back(previous.split_rows(rows));
        }
        self.shadow.seen = 0;
        self.top += i64::from(rows);
        self.trim();
    }

    /// Forget kept rows that have scrolled past the top of the screen: they
    /// belong to the scrollback now.
    fn trim(&mut self) {
        let mut excess = kept_len(&self.kept) - self.top.max(0);
        while excess > 0 {
            let Some(oldest) = self.kept.front_mut() else {
                break;
            };
            let height = i64::from(oldest.area().height);
            if height <= excess {
                self.kept.pop_front();
            } else {
                oldest.drop_rows(excess as u16);
            }
            excess -= height;
        }
    }

    /// Draw a frame as wide as the screen and of any height, as one
    /// synchronized update in a single write. `height` is the screen height
    /// in rows. Returns whether anything was written; an unchanged frame
    /// writes nothing. A failed write invalidates positions.
    pub fn draw(
        &mut self,
        writer: &mut impl Write,
        frame: &Buffer,
        height: u16,
    ) -> io::Result<bool> {
        let wrote = !self.render(frame, height).is_empty();
        self.shadow
            .send(writer)
            .inspect_err(|_| self.invalidate())?;
        Ok(wrote)
    }

    /// The bytes that bring the screen from the last frame to `frame`, as
    /// one synchronized update; empty when nothing differs. The same contract
    /// as `Renderer::render`: send them in one write, and call `invalidate`
    /// if it fails.
    pub fn render(&mut self, frame: &Buffer, height: u16) -> &[u8] {
        if self.current(frame, height) {
            self.shadow.output.clear();
            return &self.shadow.output;
        }
        let rows = i64::from(height.max(1));
        let len = i64::from(frame.area().height);
        let screen = Some((frame.area().width, height));
        let redraw = self.redraw || self.screen.is_some_and(|old| Some(old) != screen);
        if self.screen.is_none() {
            self.top = self.top.min(rows - 1);
        }
        self.screen = screen;
        let previous = self.shadow.previous.take();
        let mut old = previous.as_ref().filter(|_| !redraw);
        let old_len = old.map_or(0, |old| i64::from(old.area().height));
        let reachable = (-self.top).max(0);
        let first_changed =
            (reachable..len.max(old_len)).find(|&index| !same_row(frame, old, index));
        let cursor_changed = self.shadow.cursor_changed(frame, old);
        if first_changed.is_none() && !cursor_changed && !redraw {
            self.shadow.keep(frame, previous);
            return &self.shadow.output;
        }
        let mut pen = self.shadow.begin();
        // Screen rows that may hold stale content from the last frame.
        let mut stale = self.top + old_len;
        if redraw || len < old_len && (self.top + len <= 0 || len >= rows) {
            self.repaint_kept(&mut pen, frame, rows);
            old = None;
            stale = rows;
        }
        let top = self.top.min(rows - len);
        let scroll = self.top - top;
        match first_changed.filter(|first| scroll > 0 && top + first < 0) {
            Some(first) => self.flow(&mut pen, frame, rows, first),
            None => self.rewrite(&mut pen, frame, old, rows, scroll, stale),
        }
        self.top = top;
        let cursor = frame.cursor().and_then(|(x, y)| {
            let y = self.top + i64::from(y);
            (0..rows).contains(&y).then_some((x, y as u16))
        });
        self.shadow.finish(pen, frame, previous, cursor);
        self.redraw = false;
        self.trim();
        &self.shadow.output
    }

    /// Positions are unknown, or a shrinking frame that fills the screen
    /// keeps its last row at the bottom so every visible row moves. Either
    /// way the tail is placed at the top of the screen beneath the committed
    /// rows that fit, or ending at its bottom, and the committed rows are
    /// written again from the top. Every visible row is then stale.
    fn repaint_kept(&mut self, pen: &mut Pen, frame: &Buffer, rows: i64) {
        let len = i64::from(frame.area().height);
        self.top = (rows - len).min(kept_len(&self.kept));
        let width = usize::from(frame.area().width);
        let skip = kept_len(&self.kept) - self.top.max(0);
        let output = &mut self.shadow.output;
        let visible = self
            .kept
            .iter()
            .flat_map(|kept| (0..kept.area().height).map(move |y| (kept, kept.row(y))));
        for (screen_row, (kept, row)) in visible.skip(skip as usize).enumerate() {
            let _ = write!(output, "\r\x1b[{};1H", screen_row + 1);
            // The terminal has reflowed rows kept from another width, if
            // it reflows at all; they are cropped or padded, not rewrapped.
            let row = crop(row, width);
            pen.row(output, kept, row);
            if row.len() < width {
                output.extend_from_slice(b"\x1b[K");
            }
        }
    }

    /// Flow: print from the first changed row and let the terminal scroll,
    /// so every row reaches history with its final content. A row one past
    /// the bottom scrolls in with one newline.
    fn flow(&mut self, pen: &mut Pen, frame: &Buffer, rows: i64, first: i64) {
        let len = i64::from(frame.area().height);
        let output = &mut self.shadow.output;
        let position = self.top + first;
        if position >= rows {
            let _ = write!(output, "\r\x1b[{rows};1H\n");
        } else {
            let _ = write!(output, "\r\x1b[{};1H", position + 1);
        }
        for (i, index) in (first..len).enumerate() {
            if i > 0 {
                output.extend_from_slice(b"\r\n");
            }
            pen.row(output, frame, frame.row(index as u16));
        }
    }

    /// Scroll growth in with newlines, then rewrite only the rows that
    /// differ from the last frame, and the stale rows beneath a frame that
    /// shrank.
    fn rewrite(
        &mut self,
        pen: &mut Pen,
        frame: &Buffer,
        old: Option<&Buffer>,
        rows: i64,
        scroll: i64,
        stale: i64,
    ) {
        let len = i64::from(frame.area().height);
        let old_len = old.map_or(0, |old| i64::from(old.area().height));
        let output = &mut self.shadow.output;
        if scroll > 0 {
            let _ = write!(output, "\r\x1b[{rows};1H");
            output.extend(std::iter::repeat_n(b'\n', scroll as usize));
        }
        let top = self.top - scroll;
        let stale = stale - scroll;
        let mut written = None;
        for screen_row in top.max(0)..rows.min(stale.max(top + len)) {
            let index = screen_row - top;
            if index < old_len && same_row(frame, old, index) {
                continue;
            }
            if written == Some(screen_row - 1) {
                output.extend_from_slice(b"\r\n");
            } else {
                let _ = write!(output, "\r\x1b[{};1H", screen_row + 1);
            }
            let index = index.min(i64::from(u16::MAX)) as u16;
            pen.row(output, frame, frame.row(index));
            written = Some(screen_row);
        }
    }

    /// Whether `render` would return no bytes without comparing a row:
    /// `frame` is the one last rendered, on a screen of the same size.
    pub(crate) fn current(&self, frame: &Buffer, height: u16) -> bool {
        self.shadow.drawn(frame)
            && !self.redraw
            && self.screen == Some((frame.area().width, height))
    }

    /// Park the cursor on a fresh line beneath the last frame, where a shell
    /// prompt or a child program can continue.
    pub fn finish(&mut self, writer: &mut impl Write) -> io::Result<()> {
        self.parking().write(writer)?;
        writer.flush()
    }

    /// The bytes that put the cursor on a fresh line beneath the last frame.
    /// A session keeps them current so that even a crash leaves the frame
    /// intact above whatever is printed next.
    pub fn park(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        let _ = self.parking().write(&mut bytes);
        bytes
    }

    /// Where `park` puts the cursor, as numbers that are cheap to keep current.
    pub(crate) fn parking(&self) -> Park {
        let Some((_, rows)) = self.screen else {
            // No frame has moved the launch cursor.
            return Park::default();
        };
        let rows = i64::from(rows.max(1));
        let (row, newline) = match self.len() {
            0 if self.top < rows => ((self.top + 1).clamp(1, rows), false),
            // Committing the bottom row leaves the next row just off screen.
            0 => (rows, true),
            len => ((self.top + len).clamp(1, rows), true),
        };
        Park {
            row: Some(row as u16),
            newline,
        }
    }

    /// Rows in the last frame, less any committed.
    fn len(&self) -> i64 {
        self.shadow
            .previous
            .as_ref()
            .map_or(0, |old| i64::from(old.area().height))
    }

    /// The frame row shown at a screen row, for mapping a mouse report onto
    /// the frame. `None` when the row is above or below the frame.
    pub fn frame_row(&self, screen_row: u16) -> Option<u16> {
        let row = i64::from(screen_row) - self.top;
        (0..self.len()).contains(&row).then_some(row as u16)
    }
}

/// Whether a row of `frame` is unchanged from the same row of `old`.
fn same_row(frame: &Buffer, old: Option<&Buffer>, index: i64) -> bool {
    old.is_some_and(|old| frame.same_row(old, index as u16))
}

/// How many committed rows are kept.
fn kept_len(kept: &VecDeque<Buffer>) -> i64 {
    kept.iter().map(|rows| i64::from(rows.area().height)).sum()
}

/// A row cut to `width` cells without splitting a wide grapheme.
fn crop(row: &[Slot], width: usize) -> &[Slot] {
    if row.len() <= width {
        return row;
    }
    let mut end = width;
    // A continuation at the edge means the grapheme it continues is cut.
    while end > 0 && row[end].width == 0 {
        end -= 1;
    }
    let cut = row[width].width == 0;
    &row[..if cut { end } else { width }]
}

/// Where to leave the cursor when a session ends: on a one-based screen row,
/// then on a fresh line beneath it. The default moves nothing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Park {
    pub row: Option<u16>,
    pub newline: bool,
}

impl Park {
    /// Write the cursor movement. It needs no terminal state, so it is safe
    /// to send from a panic hook or signal thread.
    pub fn write(self, output: &mut impl Write) -> io::Result<()> {
        if let Some(row) = self.row {
            write!(output, "\r\x1b[{row};1H")?;
        }
        if self.newline {
            output.write_all(b"\r\n")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cropping_never_splits_a_wide_grapheme() {
        let mut row = Buffer::new(6, 1);
        row.write(row.area(), "abc漢d", crate::Style::default());
        assert_eq!(crop(row.row(0), 4).len(), 3);
        assert_eq!(crop(row.row(0), 5).len(), 5);
        assert_eq!(crop(row.row(0), 8).len(), 6);
    }

    #[test]
    fn a_forgotten_cursor_is_shown_again_on_an_unchanged_frame() {
        let mut frame = Buffer::new(4, 1);
        frame.cursor = Some((1, 0));
        frame.shape = crate::CursorShape::Bar;
        let mut inline = Inline::new(0, Depth::Rgb);
        inline.draw(&mut Vec::new(), &frame, 3).unwrap();
        inline.forget_cursor();
        let mut output = Vec::new();
        inline.draw(&mut output, &frame, 3).unwrap();
        let output = String::from_utf8(output).unwrap();
        assert!(
            output.ends_with("\x1b[6 q\r\x1b[1;2H\x1b[?25h\x1b[?2026l"),
            "{output:?}"
        );
    }
}
