//! A keyboard-selectable list with a viewport that follows selection.
use crate::{Canvas, Element, Event, Key, Response, Style};
use unicode_width::UnicodeWidthStr;

#[derive(Default)]
pub struct Select {
    pub items: Vec<String>,
    pub selected: usize,
    pub style: Style,
    pub highlight: Style,
}

impl Select {
    pub fn new(items: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            items: items.into_iter().map(Into::into).collect(),
            highlight: Style {
                reverse: true,
                ..Style::default()
            },
            ..Self::default()
        }
    }
}
impl Element for Select {
    fn focusable(&self) -> bool {
        !self.items.is_empty()
    }
    fn measure(&self, _: Option<u16>) -> (u16, u16) {
        (
            self.items
                .iter()
                .map(|s| s.width())
                .max()
                .unwrap_or(0)
                .min(u16::MAX as usize) as u16,
            self.items.len().min(u16::MAX as usize) as u16,
        )
    }
    fn paint(&self, canvas: &mut Canvas<'_>) {
        let height = usize::from(canvas.size().1);
        let selected = self.selected.min(self.items.len().saturating_sub(1));
        let offset = selected.saturating_sub(height.saturating_sub(1));
        for (y, (i, item)) in self
            .items
            .iter()
            .enumerate()
            .skip(offset)
            .take(height)
            .enumerate()
        {
            canvas.text(
                0,
                y as i32,
                item,
                if i == selected {
                    self.highlight
                } else {
                    self.style
                },
            );
        }
    }
    fn event(&mut self, event: &Event) -> Response {
        if self.items.is_empty() {
            return Response::IGNORE;
        }
        self.selected = self.selected.min(self.items.len() - 1);
        match event {
            Event::Key(Key::Up, _) => self.selected = self.selected.saturating_sub(1),
            Event::Key(Key::Down, _) => {
                self.selected = (self.selected + 1).min(self.items.len() - 1)
            }
            Event::Key(Key::Home, _) => self.selected = 0,
            Event::Key(Key::End, _) => self.selected = self.items.len() - 1,
            _ => return Response::IGNORE,
        }
        Response::CHANGED
    }
}
