//! Grapheme-safe text measurement and optional hard wrapping.
use crate::{Canvas, Element, Style};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

#[derive(Default)]
pub struct Text {
    pub content: String,
    pub style: Style,
    pub wrap: bool,
}

impl Text {
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            ..Self::default()
        }
    }

    /// Hard wrapping and measurement share the same rows so layout matches paint.
    fn rows(&self, width: Option<u16>) -> Vec<String> {
        let limit = if self.wrap {
            width.map(usize::from)
        } else {
            None
        };
        let mut rows = Vec::new();
        for line in self.content.split('\n') {
            let mut row = String::new();
            let mut cols = 0;
            for g in line.graphemes(true) {
                if g.chars().any(char::is_control) {
                    continue;
                }
                let n = g.width();
                if limit.is_some_and(|w| n > w) {
                    continue;
                }
                if limit.is_some_and(|w| cols + n > w) {
                    rows.push(std::mem::take(&mut row));
                    cols = 0;
                }
                row.push_str(g);
                cols += n;
            }
            rows.push(row);
        }
        rows
    }
}

impl Element for Text {
    fn measure(&self, width: Option<u16>) -> (u16, u16) {
        let rows = self.rows(width);
        (
            rows.iter()
                .map(|s| s.width())
                .max()
                .unwrap_or(0)
                .min(u16::MAX as usize) as u16,
            rows.len().min(u16::MAX as usize) as u16,
        )
    }
    fn paint(&self, canvas: &mut Canvas<'_>) {
        for (y, line) in self
            .rows(Some(canvas.size().0))
            .iter()
            .take(usize::from(canvas.size().1))
            .enumerate()
        {
            canvas.text(0, y as i32, line, self.style);
        }
    }
}
