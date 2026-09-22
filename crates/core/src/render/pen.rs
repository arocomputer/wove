//! Frame output shared by the full-screen and inline renderers.
use super::buffer::{Buffer, Color, CursorShape, Slot, Style};
use std::{
    io::{self, Write},
    sync::Arc,
};

/// The synchronized-update markers around a frame.
const BEGIN: &[u8] = b"\x1b[?2026h";
const END: &[u8] = b"\x1b[?2026l";

/// How many colors the output accepts. Richer colors are mapped to the nearest
/// one available; attributes such as bold are always kept.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Depth {
    /// 24-bit color.
    #[default]
    Rgb,
    /// The 256-color palette.
    Indexed,
    /// The 16 ANSI colors.
    Basic,
    /// No color at all.
    Mono,
}

impl Depth {
    /// Read the conventional hints from the process environment.
    pub fn detect() -> Self {
        Self::from_env(|name| std::env::var(name).ok())
    }

    /// `detect` against any environment, such as a remote peer's. It reads
    /// `NO_COLOR`, `COLORTERM`, `TERM`, and `TERM_PROGRAM`. `COLORTERM` is often
    /// lost, under sudo or over SSH, so terminals known for 24-bit color are
    /// recognized by name, and anything that is not plainly limited gets the
    /// 256-color palette rather than sixteen.
    pub fn from_env(var: impl Fn(&str) -> Option<String>) -> Self {
        let var = |name| var(name).unwrap_or_default();
        let (no_color, colorterm) = (var("NO_COLOR"), var("COLORTERM"));
        let (term, program) = (var("TERM"), var("TERM_PROGRAM"));
        let (term, program) = (term.as_str(), program.as_str());
        const RGB: [&str; 8] = [
            "direct",
            "kitty",
            "ghostty",
            "alacritty",
            "foot",
            "wezterm",
            "contour",
            "iterm",
        ];
        let named = |names: &[&str], value: &str| {
            let value = value.to_ascii_lowercase();
            names.iter().any(|name| value.contains(name))
        };
        if !no_color.is_empty() || term == "dumb" {
            Self::Mono
        } else if matches!(colorterm.as_str(), "truecolor" | "24bit")
            || named(&RGB, term)
            || named(&["iterm", "wezterm", "ghostty", "vscode"], program)
        {
            Self::Rgb
        } else if matches!(term, "linux" | "vt100" | "vt220" | "ansi" | "") {
            Self::Basic
        } else {
            Self::Indexed
        }
    }

    fn resolve(self, color: Color) -> Color {
        match (self, color) {
            (Self::Mono, _) => Color::Default,
            (Self::Indexed, Color::Rgb(r, g, b)) => Color::Indexed(nearest(16..=255, (r, g, b))),
            (Self::Basic, Color::Rgb(r, g, b)) => Color::Indexed(nearest(0..=15, (r, g, b))),
            (Self::Basic, Color::Indexed(n)) if n > 15 => Color::Indexed(nearest(0..=15, rgb(n))),
            _ => color,
        }
    }
}

/// The xterm default value of a palette index.
fn rgb(index: u8) -> (u8, u8, u8) {
    const BASIC: [(u8, u8, u8); 16] = [
        (0, 0, 0),
        (205, 0, 0),
        (0, 205, 0),
        (205, 205, 0),
        (0, 0, 238),
        (205, 0, 205),
        (0, 205, 205),
        (229, 229, 229),
        (127, 127, 127),
        (255, 0, 0),
        (0, 255, 0),
        (255, 255, 0),
        (92, 92, 255),
        (255, 0, 255),
        (0, 255, 255),
        (255, 255, 255),
    ];
    let level = |n: u8| if n == 0 { 0 } else { 55 + 40 * n };
    match index {
        0..=15 => BASIC[usize::from(index)],
        16..=231 => {
            let n = index - 16;
            (level(n / 36), level(n / 6 % 6), level(n % 6))
        }
        _ => {
            let gray = 8 + 10 * (index - 232);
            (gray, gray, gray)
        }
    }
}

/// The palette index within `range` closest to a color, by squared distance.
fn nearest(range: std::ops::RangeInclusive<u8>, (r, g, b): (u8, u8, u8)) -> u8 {
    let distance = |index: &u8| {
        let (pr, pg, pb) = rgb(*index);
        let d = |a: u8, b: u8| (i32::from(a) - i32::from(b)).pow(2);
        d(pr, r) + d(pg, g) + d(pb, b)
    };
    range.min_by_key(distance).unwrap_or(0)
}

/// Tracks the style and hyperlink the terminal currently has open, so runs of
/// similar cells are written without repeating them.
pub(crate) struct Pen {
    depth: Depth,
    style: Option<Style>,
    link: Option<Arc<str>>,
}

impl Pen {
    pub fn new(depth: Depth) -> Self {
        Self {
            depth,
            style: None,
            link: None,
        }
    }

    pub fn cell(&mut self, out: &mut Vec<u8>, buffer: &Buffer, cell: &Slot) {
        let link = buffer.link(cell);
        if self.link.as_ref() != link {
            self.link = link.cloned();
            match link.map(|url| &**url).filter(|url| linkable(url)) {
                Some(url) => {
                    let _ = write!(out, "\x1b]8;id={:x};{url}\x1b\\", hash(url));
                }
                None => out.extend_from_slice(b"\x1b]8;;\x1b\\"),
            }
        }
        let style = cell.style();
        if self.style != Some(style) {
            self.style = Some(style);
            self.sgr(out, style);
        }
        out.extend_from_slice(buffer.symbol(cell).as_bytes());
    }

    /// Write a whole row from the cursor, which must be at the row's first
    /// column. Trailing blanks are erased instead of written. A full row
    /// leaves the cursor on its last cell, where erasing to the end of the
    /// line would eat that cell, so full rows need no erase.
    pub fn row(&mut self, out: &mut Vec<u8>, buffer: &Buffer, cells: &[Slot]) {
        let used = trimmed(cells);
        for (x, cell) in used.iter().enumerate().filter(|(_, cell)| cell.width > 0) {
            self.cell(out, buffer, cell);
            if !cell.is_ascii() {
                // Terminals disagree on cluster widths; say where the next cell is.
                let _ = write!(out, "\x1b[{}G", x + usize::from(cell.width) + 1);
            }
        }
        self.reset(out);
        if used.len() < cells.len() || cells.is_empty() {
            out.extend_from_slice(b"\x1b[K");
        }
    }

    /// Close any hyperlink and return to the terminal's default attributes.
    pub fn reset(&mut self, out: &mut Vec<u8>) {
        if self.link.take().is_some() {
            out.extend_from_slice(b"\x1b]8;;\x1b\\");
        }
        if self.style.take().is_some_and(|s| s != Style::default()) {
            out.extend_from_slice(b"\x1b[0m");
        }
    }

    /// Replace all terminal attributes, preventing style leakage between cells.
    fn sgr(&self, out: &mut Vec<u8>, style: Style) {
        out.extend_from_slice(b"\x1b[0");
        for (on, code) in [
            (style.bold, 1),
            (style.dim, 2),
            (style.italic, 3),
            (style.underline, 4),
            (style.reverse, 7),
            (style.strikethrough, 9),
        ] {
            if on {
                let _ = write!(out, ";{code}");
            }
        }
        for (color, base) in [(style.fg, 30), (style.bg, 40)] {
            match self.depth.resolve(color) {
                Color::Default | Color::Reset => {}
                Color::Indexed(n) if n < 8 && self.depth == Depth::Basic => {
                    let _ = write!(out, ";{}", base + u16::from(n));
                }
                Color::Indexed(n) if n < 16 && self.depth == Depth::Basic => {
                    let _ = write!(out, ";{}", base + 52 + u16::from(n));
                }
                Color::Indexed(n) => {
                    let _ = write!(out, ";{};5;{n}", base + 8);
                }
                Color::Rgb(r, g, b) => {
                    let _ = write!(out, ";{};2;{r};{g};{b}", base + 8);
                }
            }
        }
        out.push(b'm');
    }
}

/// Hyperlink targets are written into an escape sequence, so they must not be
/// able to end it.
fn linkable(url: &str) -> bool {
    !url.is_empty() && url.len() <= 2048 && !url.chars().any(char::is_control)
}

/// FNV-1a. Equal targets share an id, so a wrapped link highlights as one.
fn hash(text: &str) -> u32 {
    text.bytes().fold(0x811c_9dc5, |hash, byte| {
        (hash ^ u32::from(byte)).wrapping_mul(0x0100_0193)
    })
}

/// The cells of a row up to its last non-blank one.
fn trimmed(row: &[Slot]) -> &[Slot] {
    let end = row
        .iter()
        .rposition(|cell| !cell.is_blank())
        .map_or(0, |i| i + 1);
    &row[..end]
}

/// What a renderer remembers of the terminal between frames, and the storage
/// it builds each frame's bytes in. Both renderers write a frame the same way:
/// `begin`, then their changed cells, then `finish`, or `keep` when nothing
/// on screen differs from the frame.
pub(crate) struct Shadow {
    /// The last frame written. A renderer takes it while comparing and hands
    /// it back to `finish` or `keep`, which reuse its storage.
    pub previous: Option<Buffer>,
    /// The version of the last frame drawn; zero when unknown.
    pub seen: u64,
    /// The cursor shape the terminal was last told; `None` when unknown.
    shape: Option<CursorShape>,
    pub output: Vec<u8>,
    depth: Depth,
}

impl Shadow {
    pub fn new(depth: Depth) -> Self {
        Self {
            previous: None,
            seen: 0,
            shape: None,
            output: Vec::new(),
            depth,
        }
    }

    /// Whether `frame` is the one last written, so drawing it again can be
    /// skipped without comparing a cell.
    pub fn drawn(&self, frame: &Buffer) -> bool {
        frame.version != 0 && frame.version == self.seen
    }

    /// Whether the terminal's cursor differs from what `frame` wants, given
    /// the frame it last showed, if known.
    pub fn cursor_changed(&self, frame: &Buffer, old: Option<&Buffer>) -> bool {
        Some(frame.cursor_shape()) != self.shape
            || old.is_none_or(|old| old.cursor() != frame.cursor())
    }

    /// Something else may have hidden or reshaped the cursor, such as leaving
    /// the session or the alternate screen. The next frame sends it again.
    pub fn forget_cursor(&mut self) {
        self.shape = None;
        self.seen = 0;
    }

    /// Start a frame's bytes: a synchronized update, with the cursor hidden
    /// while cells change beneath it.
    pub fn begin(&mut self) -> Pen {
        self.output.clear();
        self.output.extend_from_slice(BEGIN);
        self.output.extend_from_slice(b"\x1b[?25l");
        Pen::new(self.depth)
    }

    /// Record `frame` as drawn without writing anything.
    pub fn keep(&mut self, frame: &Buffer, previous: Option<Buffer>) {
        self.previous = previous;
        self.seen = frame.version;
    }

    /// End the frame begun with `begin`: set the cursor shape, show the cursor
    /// at the zero-based screen `cursor` if there is one, and send the frame
    /// in one write, so nothing else that writes to the terminal can land
    /// inside it. On success `frame` becomes the shadow frame, reusing the
    /// storage of `previous`. On failure what the terminal shows is unknown,
    /// so the shadow frame and cursor are forgotten.
    pub fn finish(
        &mut self,
        writer: &mut impl Write,
        mut pen: Pen,
        frame: &Buffer,
        mut previous: Option<Buffer>,
        cursor: Option<(u16, u16)>,
    ) -> io::Result<()> {
        let output = &mut self.output;
        pen.reset(output);
        let shape = Some(frame.cursor_shape());
        if shape != self.shape {
            write!(output, "\x1b[{} q", frame.cursor_shape().code())?;
        }
        if let Some((x, y)) = cursor {
            let (x, y) = (u32::from(x) + 1, u32::from(y) + 1);
            write!(output, "\r\x1b[{y};{x}H\x1b[?25h")?;
        }
        output.extend_from_slice(END);
        if let Err(error) = writer.write_all(output).and_then(|()| writer.flush()) {
            self.previous = None;
            self.forget_cursor();
            return Err(error);
        }
        self.shape = shape;
        match &mut previous {
            Some(buffer) => buffer.clone_from(frame),
            None => previous = Some(frame.clone()),
        }
        self.keep(frame, previous);
        Ok(())
    }
}
