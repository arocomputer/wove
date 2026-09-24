//! A clipped vertical viewport with explicit scrolling and optional tail following.
use crate::{Canvas, Element, Event, Key, Layout, MouseKind, Response, Style};

/// Children scrolled out of view are not painted, so content may be far taller
/// than the viewport. A descendant that receives focus is scrolled into view.
#[derive(Default)]
pub struct Scroll {
    pub offset: u32,
    pub follow: bool,
    /// Draws a scrollbar in a column reserved at the right edge. Set it
    /// through `Scroll::with_bar`, because the column is part of the layout.
    bar: Option<Style>,
    limit: u32,
    page: u32,
    /// Content rows to bring into view at the next viewport.
    reveal: Option<(u32, u32)>,
}

impl Scroll {
    /// A scroll with a scrollbar whose thumb shows the position and the share
    /// of the content in view. The bar is hidden while everything fits.
    pub fn with_bar(style: Style) -> Self {
        Self {
            bar: Some(style),
            ..Self::default()
        }
    }
}

impl Element for Scroll {
    fn focusable(&self) -> bool {
        true
    }
    fn layout(&self) -> Layout {
        Layout {
            overflow: taffy::Point {
                x: taffy::Overflow::Hidden,
                y: taffy::Overflow::Scroll,
            },
            flex_direction: taffy::FlexDirection::Column,
            padding: taffy::Rect {
                right: taffy::LengthPercentage::length(f32::from(u8::from(self.bar.is_some()))),
                ..taffy::Rect::zero()
            },
            ..Layout::default()
        }
    }
    fn overlay(&self, canvas: &mut Canvas<'_>) {
        let (width, height) = canvas.size();
        let Some(style) = self.bar.filter(|_| self.limit > 0 && width > 0) else {
            return;
        };
        let rows = u64::from(height);
        let content = u64::from(self.limit) + u64::from(self.page);
        let thumb = (rows * rows / content.max(1)).clamp(1, rows);
        let top = u64::from(self.offset) * (rows - thumb) / u64::from(self.limit);
        for y in 0..rows {
            let inside = (top..top + thumb).contains(&y);
            let mark = if inside { "┃" } else { "│" };
            canvas.text(i32::from(width) - 1, y as i32, mark, style);
        }
    }
    fn reveal(&mut self, (_, y): (u32, u32), (_, height): (u16, u16)) {
        self.reveal = Some((y, y.saturating_add(u32::from(height))));
    }
    fn viewport(&mut self, size: (u16, u16), content: (u32, u32)) -> (u32, u32) {
        self.page = u32::from(size.1);
        self.limit = content.1.saturating_sub(self.page);
        // Move only as far as showing the rows needs, preferring their top.
        if let Some((top, bottom)) = self.reveal.take() {
            let shown = if self.follow { self.limit } else { self.offset };
            let offset = if top < shown {
                top
            } else if bottom > shown.saturating_add(self.page) {
                (bottom - self.page).min(top)
            } else {
                shown
            };
            if offset != shown {
                self.offset = offset;
                self.follow = offset >= self.limit;
            }
        }
        self.offset = if self.follow {
            self.limit
        } else {
            self.offset.min(self.limit)
        };
        (0, self.offset)
    }
    fn event(&mut self, event: &Event) -> Response {
        let old = (self.offset, self.follow);
        match event {
            Event::Key(Key::Up, _)
            | Event::Mouse(crate::Mouse {
                kind: MouseKind::ScrollUp,
                ..
            }) => self.offset = self.offset.saturating_sub(1),
            Event::Key(Key::Down, _)
            | Event::Mouse(crate::Mouse {
                kind: MouseKind::ScrollDown,
                ..
            }) => self.offset = self.offset.saturating_add(1).min(self.limit),
            Event::Key(Key::PageUp, _) => {
                self.offset = self.offset.saturating_sub(self.page.max(1))
            }
            Event::Key(Key::PageDown, _) => {
                self.offset = self.offset.saturating_add(self.page.max(1)).min(self.limit)
            }
            Event::Key(Key::Home, _) => self.offset = 0,
            Event::Key(Key::End, _) => self.offset = self.limit,
            _ => return Response::IGNORE,
        }
        // Reaching the end by any key follows new content; Home never does.
        self.follow = self.offset == self.limit && !matches!(event, Event::Key(Key::Home, _));
        // An offset that cannot move lets the event reach an outer scroll.
        if old == (self.offset, self.follow) {
            Response::IGNORE
        } else {
            Response::REPAINT
        }
    }
}
