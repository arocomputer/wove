//! A clipped cell grid that keeps grapheme clusters and wide-cell ownership intact.
use crate::Rect;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

/// Terminal colors, independent of the terminal backend.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Color {
    /// Inherit the terminal default.
    #[default]
    Default,
    /// An index in the terminal's 256-color palette.
    Indexed(u8),
    /// A true-color value.
    Rgb(u8, u8, u8),
}

/// Complete cell styling; applications choose their own palette.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Style {
    pub fg: Color,
    pub bg: Color,
    pub bold: bool,
    pub underline: bool,
    pub reverse: bool,
}

/// One terminal cell. Wide graphemes own following continuation cells.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cell {
    pub(crate) symbol: String,
    pub(crate) width: usize,
    pub(crate) style: Style,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            symbol: " ".into(),
            width: 1,
            style: Style::default(),
        }
    }
}

impl Cell {
    /// Grapheme text; an empty string denotes a wide-grapheme continuation.
    pub fn symbol(&self) -> &str {
        &self.symbol
    }
    pub fn style(&self) -> Style {
        self.style
    }
}

/// An owned frame. All text writes are clipped and reject terminal controls.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Buffer {
    width: u16,
    height: u16,
    pub(crate) cells: Vec<Cell>,
    pub(crate) cursor: Option<(u16, u16)>,
}

impl Buffer {
    /// Allocate a blank frame of the requested dimensions.
    pub fn new(width: u16, height: u16) -> Self {
        Self {
            width,
            height,
            cursor: None,
            cells: vec![Cell::default(); usize::from(width) * usize::from(height)],
        }
    }
    pub fn cursor(&self) -> Option<(u16, u16)> {
        self.cursor
    }

    /// The full drawable area.
    pub fn area(&self) -> Rect {
        Rect::new(0, 0, self.width, self.height)
    }
    /// Inspect a cell without permitting broken wide-grapheme ownership.
    pub fn cell(&self, x: u16, y: u16) -> Option<&Cell> {
        if x >= self.width || y >= self.height {
            return None;
        }
        self.cells
            .get(usize::from(y) * usize::from(self.width) + usize::from(x))
    }
    /// Restore every cell to its blank default.
    pub fn clear(&mut self) {
        self.cells.fill(Cell::default());
        self.cursor = None;
    }

    /// Erase the complete grapheme covering an index, including its trailing cells.
    fn erase(&mut self, index: usize) {
        let row = index / usize::from(self.width) * usize::from(self.width);
        let mut start = index;
        while start > row && self.cells[start].width == 0 {
            start -= 1;
        }
        let end = (start + self.cells[start].width).min(row + usize::from(self.width));
        for cell in &mut self.cells[start..end] {
            *cell = Cell::default();
        }
    }

    /// Write one line within `area`, returning columns used. Control characters
    /// and standalone zero-width graphemes are skipped. A grapheme never splits.
    /// Overwriting either half of a wide grapheme clears the whole old grapheme.
    pub fn write(&mut self, area: Rect, text: &str, style: Style) -> u16 {
        let area = area.intersection(self.area());
        if area.height == 0 || area.width == 0 {
            return 0;
        }
        let mut used = 0usize;
        for grapheme in text.graphemes(true) {
            if grapheme.chars().any(char::is_control) {
                continue;
            }
            let width = grapheme.width();
            if width == 0 {
                continue;
            }
            if used + width > usize::from(area.width) {
                break;
            }
            let index = usize::from(area.y) * usize::from(self.width) + usize::from(area.x) + used;
            for i in index..index + width {
                self.erase(i);
            }
            self.cells[index] = Cell {
                symbol: grapheme.into(),
                width,
                style,
            };
            for i in index + 1..index + width {
                self.cells[i] = Cell {
                    symbol: String::new(),
                    width: 0,
                    style,
                };
            }
            used += width;
        }
        used as u16
    }

    /// Get visible rows as plain text, useful for snapshots and diagnostics.
    pub fn lines(&self) -> Vec<String> {
        if self.width == 0 {
            return vec![String::new(); usize::from(self.height)];
        }
        self.cells
            .chunks(usize::from(self.width))
            .map(|row| row.iter().map(|c| c.symbol.as_str()).collect())
            .collect()
    }
}
