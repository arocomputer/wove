//! Reusable components. Applications can implement `Widget` for their own types.
mod input;
mod scroll;
mod select;
mod text;

use crate::{Canvas, Layout, Style, Widget};
pub use input::Input;
pub use scroll::Scroll;
pub use select::Select;
pub use text::Text;

/// A layout-only container, useful for rows, columns, and grids.
#[derive(Default)]
pub struct Container;
impl Widget for Container {}

/// A bordered container. Its default layout reserves one cell on each edge.
#[derive(Default)]
pub struct Panel {
    pub style: Style,
}
impl Widget for Panel {
    fn layout(&self) -> Layout {
        Layout {
            border: taffy::Rect::length(1.0),
            ..Layout::default()
        }
    }
    fn paint(&self, canvas: &mut Canvas<'_>) {
        canvas.border(self.style);
    }
}
