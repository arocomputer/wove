//! Shared grapheme layout for plain and styled text.
use crate::{Canvas, Style};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

/// A run of text with one complete terminal style.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Span {
    pub text: String,
    pub style: Style,
}
impl Span {
    pub fn new(text: impl Into<String>, style: Style) -> Self {
        Self {
            text: text.into(),
            style,
        }
    }
}

/// Wrapping never divides a grapheme. Word wrapping falls back to character wrapping.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Wrap {
    #[default]
    None,
    Character,
    Word,
}

struct Glyph {
    range: std::ops::Range<usize>,
    style: Style,
    width: usize,
    whitespace: bool,
}

/// Measured rows used unchanged by painting. Span boundaries cannot split graphemes.
pub struct TextLayout {
    content: String,
    rows: Vec<Vec<Glyph>>,
}
impl TextLayout {
    pub fn new(spans: &[Span], width: Option<u16>, wrap: Wrap) -> Self {
        let content: String = spans.iter().map(|s| s.text.as_str()).collect();
        let limit = if wrap == Wrap::None {
            usize::MAX
        } else {
            width.map_or(usize::MAX, usize::from)
        };
        let mut ends = Vec::new();
        let mut end = 0;
        for span in spans {
            end += span.text.len();
            ends.push(end);
        }
        let mut rows = Vec::new();
        let mut row: Vec<Glyph> = Vec::new();
        let mut columns = 0;
        for (offset, g) in content.grapheme_indices(true) {
            if g == "\n" || g == "\r\n" {
                rows.push(std::mem::take(&mut row));
                columns = 0;
                continue;
            }
            if g.chars().any(char::is_control) {
                continue;
            }
            let n = g.width();
            if n > limit {
                continue;
            }
            if columns + n > limit && !row.is_empty() {
                let split = if wrap == Wrap::Word {
                    row.iter().rposition(|g| g.whitespace).map(|i| i + 1)
                } else {
                    None
                };
                if let Some(split) = split {
                    let tail = row.split_off(split);
                    rows.push(std::mem::replace(&mut row, tail));
                    columns = row.iter().map(|g| g.width).sum();
                } else {
                    rows.push(std::mem::take(&mut row));
                    columns = 0;
                }
            }
            let index = ends.partition_point(|end| *end <= offset);
            let style = spans.get(index).map_or(Style::default(), |s| s.style);
            row.push(Glyph {
                range: offset..offset + g.len(),
                whitespace: g.chars().all(char::is_whitespace),
                style,
                width: n,
            });
            columns += n;
        }
        rows.push(row);
        Self { content, rows }
    }
    pub fn size(&self) -> (u16, u16) {
        (
            self.rows
                .iter()
                .map(|r| r.iter().map(|g| g.width).sum::<usize>())
                .max()
                .unwrap_or(0)
                .min(u16::MAX as usize) as u16,
            self.rows.len().min(u16::MAX as usize) as u16,
        )
    }
    pub fn paint(&self, canvas: &mut Canvas<'_>) {
        for (y, row) in self
            .rows
            .iter()
            .take(usize::from(canvas.size().1))
            .enumerate()
        {
            let mut x = 0;
            for glyph in row {
                canvas.text(x, y as i32, &self.content[glyph.range.clone()], glyph.style);
                x += glyph.width as i32;
            }
        }
    }
}
