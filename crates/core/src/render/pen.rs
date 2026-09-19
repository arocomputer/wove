//! Cell output shared by the full-screen and inline renderers.
use super::buffer::{Buffer, Color, Slot, Style};
use std::{io::Write, sync::Arc};

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
    /// Read the conventional environment hints: `NO_COLOR`, `COLORTERM`, `TERM`.
    pub fn detect() -> Self {
        let var = |name| std::env::var(name).unwrap_or_default();
        Self::from_hints(&var("NO_COLOR"), &var("COLORTERM"), &var("TERM"))
    }

    /// `detect` with the environment supplied, for remote peers and tests.
    pub fn from_hints(no_color: &str, colorterm: &str, term: &str) -> Self {
        if !no_color.is_empty() || term == "dumb" {
            Self::Mono
        } else if matches!(colorterm, "truecolor" | "24bit") {
            Self::Rgb
        } else if term.contains("256color") {
            Self::Indexed
        } else {
            Self::Basic
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
pub(crate) fn trimmed(row: &[Slot]) -> &[Slot] {
    let end = row
        .iter()
        .rposition(|cell| !cell.is_blank())
        .map_or(0, |i| i + 1);
    &row[..end]
}
