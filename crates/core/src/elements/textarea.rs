//! Multiline editing with a cursor-following viewport and optional soft wrapping.
use super::cluster;
use crate::{
    render::{cell_width, columns},
    text::{clean, edit, Editor},
    Canvas, Element, Event, Response, Style,
};
use unicode_segmentation::UnicodeSegmentation;

/// A multiline editor. The viewport follows the cursor. With `wrap`, lines
/// break at words to fit the width and `Up` and `Down` move through the
/// wrapped rows; without it, long lines scroll horizontally.
#[derive(Default)]
pub struct Textarea {
    pub editor: Editor,
    pub style: Style,
    /// Drawn over `style` for atoms, the editor's indivisible tokens.
    pub atom: Style,
    /// Shown while the value is empty, with the cursor at its start when focused.
    pub placeholder: String,
    pub wrap: bool,
    pub cursor: crate::CursorShape,
}
impl Textarea {
    pub fn new(value: &str) -> Self {
        Self {
            editor: Editor::new(clean(value, true)),
            ..Self::default()
        }
    }
    fn width(&self, width: u16) -> Option<u16> {
        self.wrap.then_some(width.max(1))
    }
}
impl Element for Textarea {
    fn focusable(&self) -> bool {
        true
    }
    fn measure(&self, width: Option<u16>) -> (u16, u16) {
        let (widest, rows) = self.editor.extent_at(width.and_then(|w| self.width(w)));
        (
            widest.clamp(1, usize::from(width.unwrap_or(u16::MAX)).max(1)) as u16,
            rows.min(u16::MAX as usize) as u16,
        )
    }
    /// Wrap to the width and scroll just far enough to keep the cursor in view.
    fn viewport(&mut self, (width, height): (u16, u16), _: (u32, u32)) -> (u32, u32) {
        self.editor.width = self.width(width);
        let rows = self.editor.rows();
        let cursor = self.editor.cursor();
        let row = Editor::row_of(&rows, cursor);
        let col = columns(&self.editor.text()[rows[row].start..cursor]);
        let top = row.saturating_sub(usize::from(height.max(1)) - 1);
        let left = if self.wrap {
            0
        } else {
            col.saturating_sub(usize::from(width.max(1)) - 1)
        };
        self.editor.set_view(top, left);
        (0, 0)
    }
    fn paint(&self, canvas: &mut Canvas<'_>) {
        let (width, height) = canvas.size();
        let value = self.editor.text();
        if value.is_empty() {
            canvas.text(0, 0, &self.placeholder, self.style);
        }
        let rows = self.editor.rows();
        let (top, left) = self.editor.view();
        let cursor = self.editor.cursor();
        let row = Editor::row_of(&rows, cursor);
        let col = columns(&value[rows[row].start..cursor]);
        let selection = self.editor.selection();
        let focused = canvas.focused();
        let selected = |at: usize| focused && selection.contains(&at);
        let atoms = self.editor.atoms();
        let right = left + usize::from(width);
        for (y, range) in rows.iter().enumerate().skip(top).take(usize::from(height)) {
            // Atoms are ordered and disjoint, so one pass over the row finds them.
            let mut atom = atoms.partition_point(|a| a.range.end <= range.start);
            let mut x = 0;
            for (i, g) in value[range.clone()].grapheme_indices(true) {
                if x >= right {
                    break;
                }
                let at = range.start + i;
                while atoms.get(atom).is_some_and(|a| a.range.end <= at) {
                    atom += 1;
                }
                let inside = atoms.get(atom).is_some_and(|a| a.range.contains(&at));
                let base = if inside { self.atom } else { self.style };
                let style = Style {
                    reverse: base.reverse || selected(at),
                    ..base
                };
                let cells = cell_width(g, x);
                cluster(
                    canvas,
                    x as i32 - left as i32,
                    (y - top) as i32,
                    g,
                    cells,
                    style,
                );
                x += cells;
            }
            // A selected line break shows as one reversed cell.
            if value[range.end..].starts_with('\n') && selected(range.end) {
                let style = Style {
                    reverse: true,
                    ..self.style
                };
                canvas.text(x as i32 - left as i32, (y - top) as i32, " ", style);
            }
        }
        if canvas.focused() {
            // Whitespace hung past a wrap seam leaves the cursor on the last cell.
            let x = (col - left).min(usize::from(width) - 1);
            canvas.cursor(x as i32, (row - top) as i32);
            canvas.cursor_shape(self.cursor);
        }
    }
    fn event(&mut self, event: &Event) -> Response {
        edit(&mut self.editor, event, true)
    }
}
