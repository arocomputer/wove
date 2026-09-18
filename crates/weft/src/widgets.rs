//! Small composable widgets; state and event routing stay with the application.
use crate::{Buffer, Rect, Style};

/// A component that paints into a bounded region of a frame.
pub trait Widget {
    /// Render without performing I/O or changing application state.
    fn render(&self, area: Rect, buffer: &mut Buffer);
}

/// Plain multiline text, clipped by grapheme at the right and bottom edges.
pub struct Text<'a> {
    /// Text to display; newlines delimit rows.
    pub content: &'a str,
    /// Style shared by all text cells.
    pub style: Style,
}

impl Widget for Text<'_> {
    fn render(&self, area: Rect, buffer: &mut Buffer) {
        let area = area.intersection(buffer.area());
        for (row, line) in self
            .content
            .lines()
            .take(usize::from(area.height))
            .enumerate()
        {
            buffer.write(
                Rect::new(area.x, area.y + row as u16, area.width, 1),
                line,
                self.style,
            );
        }
    }
}

/// A border around another widget; the child receives the interior region.
pub struct Border<W> {
    /// Child content.
    pub child: W,
    /// Border style.
    pub style: Style,
}

impl<W: Widget> Widget for Border<W> {
    fn render(&self, area: Rect, buffer: &mut Buffer) {
        let area = area.intersection(buffer.area());
        if area.width < 2 || area.height < 2 {
            return;
        }
        let right = area.x + area.width - 1;
        let bottom = area.y + area.height - 1;
        for x in area.x + 1..right {
            buffer.write(Rect::new(x, area.y, 1, 1), "─", self.style);
            buffer.write(Rect::new(x, bottom, 1, 1), "─", self.style);
        }
        for y in area.y + 1..bottom {
            buffer.write(Rect::new(area.x, y, 1, 1), "│", self.style);
            buffer.write(Rect::new(right, y, 1, 1), "│", self.style);
        }
        for (x, y, text) in [
            (area.x, area.y, "┌"),
            (right, area.y, "┐"),
            (area.x, bottom, "└"),
            (right, bottom, "┘"),
        ] {
            buffer.write(Rect::new(x, y, 1, 1), text, self.style);
        }
        self.child.render(area.inset(1), buffer);
    }
}

/// A list with application-owned selection and viewport offset.
pub struct List<'a> {
    /// Labels to show.
    pub items: &'a [&'a str],
    /// Absolute selected index; an out-of-range index selects nothing.
    pub selected: Option<usize>,
    /// First visible item.
    pub offset: usize,
    /// Unselected text style.
    pub style: Style,
    /// Selected text style.
    pub selected_style: Style,
}

impl Widget for List<'_> {
    fn render(&self, area: Rect, buffer: &mut Buffer) {
        let area = area.intersection(buffer.area());
        for (row, (index, item)) in self
            .items
            .iter()
            .enumerate()
            .skip(self.offset)
            .take(usize::from(area.height))
            .enumerate()
        {
            let style = if self.selected == Some(index) {
                self.selected_style
            } else {
                self.style
            };
            buffer.write(
                Rect::new(area.x, area.y + row as u16, area.width, 1),
                item,
                style,
            );
        }
    }
}
