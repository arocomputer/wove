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
    /// Draw the element, after `viewport`. Not called while nothing of it is visible.
    fn paint(&self, _canvas: &mut Canvas<'_>) {}
    /// Handle input. Mouse positions are relative to the element's top-left
    /// cell, as painted in the last frame.
    fn event(&mut self, _event: &Event) -> Response {
        Response::IGNORE
    }
    fn focusable(&self) -> bool {
        false
    }
    /// Draw over the element's children, for chrome such as a scrollbar.
    fn overlay(&self, _canvas: &mut Canvas<'_>) {}
    /// Called just before `paint`, with the element's inner size and the
    /// extent of its children's content, so an element can settle what it
    /// shows before drawing it. Returns how far the children are scrolled.
    /// Content may be far taller than a frame, so extents and offsets are
    /// 32-bit.
    fn viewport(&mut self, _size: (u16, u16), _content: (u32, u32)) -> (u32, u32) {
        (0, 0)
    }
    /// Scroll a descendant into view. Called on the nearest ancestor whose
    /// layout has `overflow: Scroll` when focus moves to a node inside it,
    /// unless a `Lazy` is nearer, before the frame is painted, with the node's
    /// position in the content as if unscrolled and its size. `viewport`
    /// follows in the same frame.
    fn reveal(&mut self, _at: (u32, u32), _size: (u16, u16)) {}
}
