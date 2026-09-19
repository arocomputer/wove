//! A clipped vertical viewport with explicit scrolling and optional tail following.
use crate::{Element, Event, Key, Layout, MouseKind, Response};

/// Children scrolled out of view are not painted, so content may be far taller
/// than the viewport.
#[derive(Default)]
pub struct Scroll {
    pub offset: u32,
    pub follow: bool,
    limit: u32,
    page: u32,
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
            ..Layout::default()
        }
    }
    fn viewport(&mut self, size: (u16, u16), content: (u32, u32)) -> (u32, u32) {
        self.page = u32::from(size.1);
        self.limit = content.1.saturating_sub(self.page);
        self.offset = if self.follow {
            self.limit
        } else {
            self.offset.min(self.limit)
        };
        (0, self.offset)
    }
    fn event(&mut self, event: &Event) -> Response {
        let old = self.offset;
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
        self.follow = self.offset == self.limit;
        Response {
            handled: old != self.offset,
            changed: old != self.offset,
        }
    }
}
