//! Built-in terminal elements. Applications can implement `Element` for their own types.
mod input;
mod pixels;
mod scroll;
mod text;

use crate::{
    render::{columns, fit},
    Border, Button, Canvas, Color, Element, Event, Key, Layout, MouseKind, Response, Style,
};
pub use input::Input;
pub use pixels::{split, Blocks, Pixels};
pub use scroll::Scroll;
pub use text::{RichText, Text};

/// The first row to show so that `selected` is within `height` rows: the
/// current `offset`, moved only as far as the selection requires. A view that
/// followed the selection instead would slide under the pointer on a click.
pub(crate) fn window(offset: usize, selected: usize, count: usize, height: usize) -> usize {
    let height = height.max(1);
    let selected = selected.min(count.saturating_sub(1));
    let first = if selected < offset {
        selected
    } else if selected >= offset + height {
        selected + 1 - height
    } else {
        offset
    };
    first.min(count.saturating_sub(height))
}

/// Move a selection among `count` rows, shown `page` at a time from `offset`,
/// for arrows, paging, Home, End, the wheel, and a left click on a row.
/// Clicks on the `header` rows above the first row, or below the last row,
/// are ignored. The offset
/// moves as `window` moves it. Navigation is consumed even at either end.
pub(crate) fn navigate(
    event: &Event,
    selected: &mut usize,
    offset: &mut usize,
    count: usize,
    page: usize,
    header: u16,
) -> Response {
    let first = window(*offset, *selected, count, page);
    let next = match event {
        Event::Mouse(mouse) => match mouse.kind {
            MouseKind::Down(Button::Left) if mouse.y >= header => {
                let row = first + usize::from(mouse.y - header);
                if row >= count {
                    return Response::IGNORE;
                }
                row
            }
            MouseKind::ScrollUp => selected.saturating_sub(1),
            MouseKind::ScrollDown => selected.saturating_add(1),
            _ => return Response::IGNORE,
        },
        Event::Key(Key::Up, _) => selected.saturating_sub(1),
        Event::Key(Key::Down, _) => selected.saturating_add(1),
        Event::Key(Key::PageUp, _) => selected.saturating_sub(page),
        Event::Key(Key::PageDown, _) => selected.saturating_add(page),
        Event::Key(Key::Home, _) => 0,
        Event::Key(Key::End, _) => count.saturating_sub(1),
        _ => return Response::IGNORE,
    }
    .min(count.saturating_sub(1));
    *offset = window(first, next, count, page);
    // A new selection changes how the rows look, never their size.
    if std::mem::replace(selected, next) == next {
        Response::HANDLED
    } else {
        Response::REPAINT
    }
}

/// Where a full view of `height` rows that ends at the last of `count` blocks
/// starts, as a block and a row within it. Positions in a column of blocks of
/// varying height take this form, because `rows`, which measures a block, is
/// only asked about the blocks a walk passes.
pub(crate) fn tail(
    count: usize,
    height: usize,
    rows: &mut dyn FnMut(usize) -> usize,
) -> (usize, usize) {
    let mut total = 0;
    for index in (0..count).rev() {
        total += rows(index);
        if total >= height {
            return (index, total - height);
        }
    }
    (0, 0)
}

/// Move a position in a column of `count` blocks by `by` rows. Moving up stops
/// at the first row; moving down stops in the last block, possibly beyond its
/// rows, so callers hold the result at the `tail`.
pub(crate) fn step(
    (mut block, mut row): (usize, usize),
    by: isize,
    count: usize,
    rows: &mut dyn FnMut(usize) -> usize,
) -> (usize, usize) {
    if by < 0 {
        let mut left = by.unsigned_abs();
        while left > row && block > 0 {
            left -= row;
            block -= 1;
            row = rows(block);
        }
        (block, row.saturating_sub(left))
    } else {
        row += by.unsigned_abs();
        while block + 1 < count {
            let extent = rows(block);
            if row < extent {
                break;
            }
            row -= extent;
            block += 1;
        }
        (block, row)
    }
}

/// Draw one grapheme that starts `cells` wide at a cell of a line. A tab
/// draws as the spaces that reach its stop in the line, which drawing it
/// alone would not.
pub(crate) fn cluster(
    canvas: &mut Canvas<'_>,
    x: i32,
    y: i32,
    g: &str,
    cells: usize,
    style: Style,
) {
    if g == "\t" {
        for i in 0..cells as i32 {
            canvas.text(x + i, y, " ", style);
        }
    } else {
        canvas.text(x, y, g, style);
    }
}

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
        let title = fit(&self.title, room);
        canvas.text(2, 0, " ", self.style);
        canvas.text(3, 0, title, self.style);
        canvas.text(3 + columns(title) as i32, 0, " ", self.style);
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
mod lazy;
pub use lazy::Lazy;
