//! A table with explicit column widths and a fixed header.
use super::{navigate, window};
use crate::{render::fit, Canvas, Element, Event, Response, Style};

/// Column widths are terminal cells. Values are clipped at grapheme boundaries;
/// headers remain visible while keyboard navigation scrolls the body.
#[derive(Default)]
pub struct Table {
    pub columns: Vec<(String, u16)>,
    pub rows: Vec<Vec<String>>,
    pub selected: usize,
    pub style: Style,
    pub header: Style,
    pub highlight: Style,
    page: usize,
    /// The first body row shown. It moves only when the selection leaves the view.
    offset: usize,
}
impl Table {
    pub fn new(columns: Vec<(String, u16)>, rows: Vec<Vec<String>>) -> Self {
        Self {
            columns,
            rows,
            header: Style {
                bold: true,
                ..Default::default()
            },
            highlight: Style {
                reverse: true,
                ..Default::default()
            },
            ..Self::default()
        }
    }
    fn row<'a>(
        &self,
        canvas: &mut Canvas<'_>,
        y: i32,
        values: impl Iterator<Item = &'a str>,
        style: Style,
    ) {
        let mut x = 0;
        for ((_, width), value) in self.columns.iter().zip(values) {
            canvas.text(x, y, fit(value, usize::from(*width)), style);
            x += i32::from(*width) + 1;
        }
    }
}
impl Element for Table {
    fn layout(&self) -> crate::Layout {
        crate::Layout {
            overflow: taffy::Point {
                x: taffy::Overflow::Hidden,
                y: taffy::Overflow::Hidden,
            },
            flex_grow: 1.0,
            ..crate::Layout::default()
        }
    }
    fn focusable(&self) -> bool {
        !self.rows.is_empty()
    }
    fn measure(&self, _: Option<u16>) -> (u16, u16) {
        (
            self.columns
                .iter()
                .fold(0u16, |n, (_, w)| n.saturating_add(*w).saturating_add(1))
                .saturating_sub(1),
            self.rows.len().saturating_add(1).min(u16::MAX as usize) as u16,
        )
    }
    fn viewport(&mut self, size: (u16, u16), _: (u32, u32)) -> (u32, u32) {
        self.page = usize::from(size.1.saturating_sub(1)).max(1);
        self.offset = window(self.offset, self.selected, self.rows.len(), self.page);
        (0, 0)
    }
    fn paint(&self, canvas: &mut Canvas<'_>) {
        self.row(
            canvas,
            0,
            self.columns.iter().map(|(name, _)| name.as_str()),
            self.header,
        );
        let selected = self.selected.min(self.rows.len().saturating_sub(1));
        for (y, (index, row)) in self
            .rows
            .iter()
            .enumerate()
            .skip(self.offset)
            .take(self.page)
            .enumerate()
        {
            self.row(
                canvas,
                y as i32 + 1,
                row.iter().map(String::as_str),
                if index == selected {
                    self.highlight
                } else {
                    self.style
                },
            );
        }
    }
    fn event(&mut self, event: &Event) -> Response {
        // Row zero is the header.
        let (count, page) = (self.rows.len(), self.page);
        navigate(event, &mut self.selected, &mut self.offset, count, page, 1)
    }
}
