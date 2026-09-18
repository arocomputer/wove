//! The contract implemented by built-in and application-defined elements.
use crate::{Canvas, Event, Layout, Response};
use std::any::Any;

/// An element measures and paints in cells. It owns its state and releases its
/// resources when removed. Events bubble to parents unless consumed.
pub trait Element: Any {
    fn layout(&self) -> Layout {
        Layout::default()
    }
    fn measure(&self, _width: Option<u16>) -> (u16, u16) {
        (0, 0)
    }
    fn paint(&self, _canvas: &mut Canvas<'_>) {}
    fn event(&mut self, _event: &Event) -> Response {
        Response::IGNORE
    }
    fn focusable(&self) -> bool {
        false
    }
    fn viewport(&mut self, _size: (u16, u16), _content: (u16, u16)) -> (u16, u16) {
        (0, 0)
    }
}
