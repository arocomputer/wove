//! The contract implemented by built-in and application-defined elements.
use crate::{Canvas, Error, Event, Id, Layout, Response};
use std::any::Any;
use std::ops::Range;

/// Access to a container's children during arrangement. Measure only the children
/// needed to choose a viewport, then place the visible ones in content coordinates.
/// Unplaced children do not paint or receive pointer input. Placement order is
/// paint order. The tree retains ownership, focus, clipping, and event bubbling.
pub trait Children {
    fn size(&self) -> (u16, u16);
    fn ids(&self) -> &[Id];
    /// The child containing a newly focused descendant that needs revealing.
    fn focused(&self) -> Option<usize>;
    /// Measure that child at the width chosen by the container, then report the
    /// descendant's position. This avoids assuming every child fills the viewport.
    fn reveal(&mut self, width: u16) -> Result<Option<ChildFocus>, Error>;
    /// Measure a child's subtree at a width, including its margins. Overlays
    /// occupy no container space and return zero; their layout belongs to the root.
    fn measure(&mut self, index: usize, width: u16) -> Result<(u32, u32), Error>;
    /// Lay out and paint a child at a signed content offset and width.
    fn place(&mut self, index: usize, at: (i32, i32), width: u16) -> Result<(), Error>;
}

/// A focus reveal request measured in a managed child's layout coordinates.
pub struct ChildFocus {
    pub index: usize,
    pub at: (u32, u32),
    pub size: (u16, u16),
}

/// An element measures and paints in cells. It owns its state and releases its
/// resources when removed. Events bubble to parents unless consumed.
pub trait Element: Any {
    /// Map local pointer coordinates to a logical text position for read-only
    /// selection. Positions belong to the element, not the painted screen.
    /// Return `None` to use screen selection. Editing elements may keep handling
    /// their own gestures instead; an unhandled press starts this selection.
    fn text_position(&self, _at: (u16, u16), _size: (u16, u16)) -> Option<usize> {
        None
    }
    /// Set a half-open logical selection, or clear it. Implementations validate
    /// positions against their content and paint their own selection highlight.
    fn select(&mut self, _range: Option<Range<usize>>) {}
    /// Copy logical text without display-only gutters or wrapping.
    fn selected_text(&self) -> Option<String> {
        None
    }
    /// Opt into application-controlled child layout. This value must remain
    /// constant for the lifetime of the element. Such containers need a size
    /// from their parent; their children do not contribute to natural sizing.
    fn manages_children(&self) -> bool {
        false
    }
    /// Arrange visible children when `manages_children` is true.
    fn arrange(&mut self, _children: &mut dyn Children) -> Result<(), Error> {
        Ok(())
    }
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
    /// unless a managed container is nearer, before painting, with the node's
    /// position in the content as if unscrolled and its size. `viewport`
    /// follows in the same frame.
    fn reveal(&mut self, _at: (u32, u32), _size: (u16, u16)) {}
}
