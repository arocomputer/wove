//! A read-only text viewport whose selection belongs to source text, not cells.
use crate::{
    text::{Cache, TextLayout, Wrap},
    Canvas, Element, Event, Key, MouseKind, Response, Style,
};
use std::ops::Range;
use unicode_segmentation::UnicodeSegmentation;

/// Scrollable text with source-byte selection and optional tail following.
/// Appending preserves a selection. Replacing text requires clearing or setting
/// selection for the new content. Wrapping and terminal resize preserve positions.
#[derive(Default)]
pub struct Document {
    pub content: String,
    pub style: Style,
    pub wrap: Wrap,
    pub top: usize,
    pub follow: bool,
    selection: Option<Range<usize>>,
    cache: Cache,
    page: usize,
    limit: usize,
}
impl Document {
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            ..Self::default()
        }
    }
    /// Source byte range selected by the user, independent of wrapping.
    pub fn selection(&self) -> Option<Range<usize>> {
        self.selection.clone()
    }
    fn rows<T>(&self, width: u16, read: impl FnOnce(&TextLayout) -> T) -> T {
        self.cache.with(
            std::iter::once((self.content.as_str(), self.style, None)),
            Some(width),
            self.wrap,
            read,
        )
    }
}
impl Element for Document {
    fn focusable(&self) -> bool {
        true
    }
    fn viewport(&mut self, size: (u16, u16), _: (u32, u32)) -> (u32, u32) {
        self.page = usize::from(size.1);
        self.limit = self
            .rows(size.0, TextLayout::rows)
            .saturating_sub(self.page);
        self.top = if self.follow {
            self.limit
        } else {
            self.top.min(self.limit)
        };
        (0, 0)
    }
    fn paint(&self, canvas: &mut Canvas<'_>) {
        self.rows(canvas.size().0, |rows| {
            rows.paint_selection(
                canvas,
                -(self.top.min(i32::MAX as usize) as i32),
                self.selection.clone(),
            )
        });
    }
    fn text_position(&self, at: (u16, u16), size: (u16, u16)) -> Option<usize> {
        Some(self.rows(size.0, |rows| {
            rows.position(
                usize::from(at.0),
                self.top.saturating_add(usize::from(at.1)),
            )
        }))
    }
    fn select(&mut self, range: Option<Range<usize>>) {
        // Snap arbitrary callers to whole graphemes as well as UTF-8 boundaries.
        let boundary = |at: usize| {
            self.content
                .grapheme_indices(true)
                .map(|(i, _)| i)
                .chain(std::iter::once(self.content.len()))
                .take_while(|i| *i <= at)
                .last()
                .unwrap_or(0)
        };
        self.selection = range.map(|range| {
            let (a, b) = (boundary(range.start), boundary(range.end));
            a.min(b)..a.max(b)
        });
    }
    fn selected_text(&self) -> Option<String> {
        self.content.get(self.selection.clone()?).map(str::to_owned)
    }
    fn event(&mut self, event: &Event) -> Response {
        let old = (self.top, self.follow);
        match event {
            Event::Key(Key::Up, _)
            | Event::Mouse(crate::Mouse {
                kind: MouseKind::ScrollUp,
                ..
            }) => self.top = self.top.saturating_sub(1),
            Event::Key(Key::Down, _)
            | Event::Mouse(crate::Mouse {
                kind: MouseKind::ScrollDown,
                ..
            }) => self.top = self.top.saturating_add(1).min(self.limit),
            Event::Key(Key::PageUp, _) => self.top = self.top.saturating_sub(self.page.max(1)),
            Event::Key(Key::PageDown, _) => {
                self.top = self.top.saturating_add(self.page.max(1)).min(self.limit)
            }
            Event::Key(Key::Home, _) => self.top = 0,
            Event::Key(Key::End, _) => self.top = self.limit,
            _ => return Response::IGNORE,
        }
        self.follow = self.top == self.limit && !matches!(event, Event::Key(Key::Home, _));
        if old == (self.top, self.follow) {
            Response::IGNORE
        } else {
            Response::REPAINT
        }
    }
}
