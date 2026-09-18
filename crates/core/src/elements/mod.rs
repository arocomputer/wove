//! Built-in terminal elements. Applications can implement `Element` for their own types.
mod input;
mod scroll;
mod select;
mod text;

use crate::{Canvas, Element, Layout, Style};
pub use input::Input;
pub use scroll::Scroll;
pub use select::Select;
pub use text::{RichText, Text};

/// A layout-only container, useful for rows, columns, and grids.
#[derive(Default)]
pub struct Container;
impl Element for Container {}

/// A bordered container. Its default layout reserves one cell on each edge.
#[derive(Default)]
pub struct Panel {
    pub style: Style,
}
impl Element for Panel {
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
mod textarea;
pub use textarea::Textarea;
mod list;
mod table;
pub use list::List;
pub use table::Table;
