//! Multiline editing with logical line navigation and a cursor-following viewport.
use crate::{
    text::{clean, edit, Editor},
    Canvas, Element, Event, Response, Style,
};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

/// An unwrapped multiline editor. The viewport follows the cursor in both axes.
#[derive(Default)]
pub struct Textarea {
    pub editor: Editor,
    pub style: Style,
    pub placeholder: String,
}
impl Textarea {
    pub fn new(value: &str) -> Self {
        Self {
            editor: Editor::new(clean(value, true)),
            ..Self::default()
        }
    }
}
impl Element for Textarea {
    fn focusable(&self) -> bool {
        true
    }
    fn measure(&self, _: Option<u16>) -> (u16, u16) {
        let lines: Vec<_> = self.editor.text().split('\n').collect();
        (
            lines
                .iter()
                .map(|line| line.width())
                .max()
                .unwrap_or(0)
                .max(1)
                .min(u16::MAX as usize) as u16,
            lines.len().min(u16::MAX as usize) as u16,
        )
    }
    fn paint(&self, canvas: &mut Canvas<'_>) {
        let (width, height) = canvas.size();
        if width == 0 || height == 0 {
            return;
        }
        let value = self.editor.text();
        if value.is_empty() && !canvas.focused() {
            canvas.text(0, 0, &self.placeholder, self.style);
            return;
        }
        let prefix = &value[..self.editor.cursor()];
        let row = prefix.bytes().filter(|b| *b == b'\n').count();
        let col = prefix.rsplit('\n').next().unwrap_or("").width();
        let top = row.saturating_sub(usize::from(height) - 1);
        let left = col.saturating_sub(usize::from(width) - 1);
        let selection = self.editor.selection();
        let mut offset = 0;
        for (y, line) in value.split('\n').enumerate() {
            if y >= top && y < top + usize::from(height) {
                let mut x = 0;
                for (i, g) in line.grapheme_indices(true) {
                    let style = Style {
                        reverse: self.style.reverse
                            || (canvas.focused() && selection.contains(&(offset + i))),
                        ..self.style
                    };
                    canvas.text(x as i32 - left as i32, (y - top) as i32, g, style);
                    x += g.width();
                }
                if canvas.focused() && selection.contains(&(offset + line.len())) {
                    canvas.text(
                        x as i32 - left as i32,
                        (y - top) as i32,
                        " ",
                        Style {
                            reverse: true,
                            ..self.style
                        },
                    );
                }
            }
            offset += line.len() + 1;
        }
        if canvas.focused() {
            canvas.cursor((col - left) as i32, (row - top) as i32);
        }
    }
    fn event(&mut self, event: &Event) -> Response {
        edit(&mut self.editor, event, true)
    }
}
