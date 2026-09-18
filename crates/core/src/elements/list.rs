//! A fixed-height list that asks its row provider only for visible rows.
use crate::{Canvas, Element, Event, Key, Response, Style};

/// Rows have one terminal line each. The application owns the underlying data;
/// `count` and `row` allow lists larger than terminal coordinate limits.
pub struct List {
    pub count: usize,
    pub selected: usize,
    pub width: u16,
    pub style: Style,
    pub highlight: Style,
    row: Box<dyn Fn(usize) -> String>,
    page: usize,
}
impl List {
    pub fn new(count: usize, width: u16, row: impl Fn(usize) -> String + 'static) -> Self {
        Self {
            count,
            selected: 0,
            width,
            style: Style::default(),
            highlight: Style {
                reverse: true,
                ..Default::default()
            },
            row: Box::new(row),
            page: 1,
        }
    }
}
impl Element for List {
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
        self.count > 0
    }
    fn measure(&self, _: Option<u16>) -> (u16, u16) {
        (self.width, self.count.min(u16::MAX as usize) as u16)
    }
    fn viewport(&mut self, size: (u16, u16), _: (u16, u16)) -> (u16, u16) {
        self.page = usize::from(size.1).max(1);
        (0, 0)
    }
    fn paint(&self, canvas: &mut Canvas<'_>) {
        let height = usize::from(canvas.size().1);
        let selected = self.selected.min(self.count.saturating_sub(1));
        let offset = selected.saturating_sub(height.saturating_sub(1));
        for (y, index) in (offset..self.count).take(height).enumerate() {
            let style = if index == selected {
                self.highlight
            } else {
                self.style
            };
            canvas.text(0, y as i32, &(self.row)(index), style);
        }
    }
    fn event(&mut self, event: &Event) -> Response {
        let old = self.selected;
        match event {
            Event::Key(Key::Up, _) => self.selected = self.selected.saturating_sub(1),
            Event::Key(Key::Down, _) => self.selected = self.selected.saturating_add(1),
            Event::Key(Key::PageUp, _) => self.selected = self.selected.saturating_sub(self.page),
            Event::Key(Key::PageDown, _) => self.selected = self.selected.saturating_add(self.page),
            Event::Key(Key::Home, _) => self.selected = 0,
            Event::Key(Key::End, _) => self.selected = self.count.saturating_sub(1),
            _ => return Response::IGNORE,
        }
        self.selected = self.selected.min(self.count.saturating_sub(1));
        Response {
            handled: true,
            changed: self.selected != old,
        }
    }
}
