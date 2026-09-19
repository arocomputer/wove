//! Built-in terminal elements. Applications can implement `Element` for their own types.
mod input;
mod scroll;
mod select;
mod text;

use crate::{render::graphemes, Border, Canvas, Color, Element, Layout, Style};
pub use input::Input;
pub use scroll::Scroll;
pub use select::Select;
pub use text::{RichText, Text};

/// A layout-only container, useful for rows, columns, and grids.
#[derive(Default)]
pub struct Container;
impl Element for Container {}

/// A bordered container. Its default layout reserves one cell on each edge.
/// A background color in `style` fills the panel, which makes it opaque over
/// whatever it overlaps. The title sits in the top edge and is cut to fit.
#[derive(Default)]
pub struct Panel {
    pub style: Style,
    pub border: Border,
    pub title: String,
}
impl Element for Panel {
    fn layout(&self) -> Layout {
        Layout {
            border: taffy::Rect::length(1.0),
            ..Layout::default()
        }
    }
    fn paint(&self, canvas: &mut Canvas<'_>) {
        if self.style.bg != Color::Default {
            canvas.fill(Style {
                bg: self.style.bg,
                ..Style::default()
            });
        }
        canvas.border(self.style, self.border);
        // Leave the corners and one cell of edge on each side of the title.
        let room = usize::from(canvas.size().0).saturating_sub(6);
        if self.title.is_empty() || room == 0 || canvas.size().1 < 2 {
            return;
        }
        let mut used = 0;
        let title: String = graphemes(&self.title)
            .take_while(|(_, width)| {
                used += width;
                used <= room
            })
            .map(|(grapheme, _)| grapheme)
            .collect();
        canvas.text(2, 0, &format!(" {title} "), self.style);
    }
}
mod textarea;
pub use textarea::Textarea;
mod feed;
mod list;
mod table;
pub use feed::Feed;
pub use list::List;
pub use table::Table;
