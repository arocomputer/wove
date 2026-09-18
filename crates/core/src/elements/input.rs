//! Single-line editing with selection and a horizontally scrolling cursor.
use crate::{text::Editor, Canvas, Element, Event, Layout, Response, Style};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

#[derive(Default)]
pub struct Input {
    pub editor: Editor,
    pub style: Style,
    pub placeholder: String,
}

impl Input {
    pub fn new(value: &str) -> Self {
        Self {
            editor: Editor::new(
                value
                    .chars()
                    .filter(|c| !c.is_control())
                    .collect::<String>(),
            ),
            ..Self::default()
        }
    }
}

impl Element for Input {
    fn focusable(&self) -> bool {
        true
    }
    fn layout(&self) -> Layout {
        Layout {
            size: taffy::Size {
                width: taffy::prelude::auto(),
                height: taffy::Dimension::length(1.0),
            },
            flex_shrink: 0.0,
            ..Layout::default()
        }
    }
    fn measure(&self, _: Option<u16>) -> (u16, u16) {
        (
            self.editor.text().width().min(u16::MAX as usize).max(1) as u16,
            1,
        )
    }
    fn paint(&self, canvas: &mut Canvas<'_>) {
        let width = usize::from(canvas.size().0);
        if width == 0 {
            return;
        }
        let value = self.editor.text();
        if value.is_empty() && !canvas.focused() {
            canvas.text(0, 0, &self.placeholder, self.style);
            return;
        }
        let cursor_col = value[..self.editor.cursor()].width();
        let desired = cursor_col.saturating_sub(width - 1);
        let mut start_col = 0;
        let mut start = 0;
        for (i, g) in value.grapheme_indices(true) {
            if start_col >= desired {
                break;
            }
            start_col += g.width();
            start = i + g.len();
        }
        let selection = self.editor.selection();
        let mut x = 0;
        for (i, g) in value[start..].grapheme_indices(true) {
            let style = Style {
                reverse: self.style.reverse
                    || (canvas.focused() && selection.contains(&(start + i))),
                ..self.style
            };
            canvas.text(x, 0, g, style);
            x += g.width() as i32;
        }
        if canvas.focused() {
            canvas.cursor(cursor_col.saturating_sub(start_col) as i32, 0);
        }
    }
    fn event(&mut self, event: &Event) -> Response {
        crate::text::edit(&mut self.editor, event, false)
    }
}
