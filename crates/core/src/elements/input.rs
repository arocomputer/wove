//! Single-line editing with selection and a horizontally scrolling cursor.
use super::cluster;
use crate::{
    render::{cell_width, columns},
    text::{clean, command, Command, Editor},
    Canvas, CursorShape, Element, Event, Layout, Response, Style,
};
use unicode_segmentation::UnicodeSegmentation;

#[derive(Default)]
pub struct Input {
    pub editor: Editor,
    pub style: Style,
    /// Shown while the value is empty, with the cursor at its start when focused.
    pub placeholder: String,
    /// Shown in place of every grapheme, for secrets. Clicks are ignored while
    /// set, because cell positions no longer say anything about the text.
    pub mask: Option<char>,
    /// The most graphemes the input accepts; typing and pasting stop there.
    pub limit: Option<usize>,
    pub cursor: CursorShape,
}

impl Input {
    /// An input holding `value`, with tabs expanded and other controls removed.
    pub fn new(value: &str) -> Self {
        Self {
            editor: Editor::new(clean(value, false)),
            ..Self::default()
        }
    }
    /// The cells a grapheme takes as displayed, starting `column` cells in.
    fn cells(&self, grapheme: &str, column: usize) -> usize {
        match self.mask {
            Some(mask) => columns(mask.encode_utf8(&mut [0; 4])),
            None => cell_width(grapheme, column),
        }
    }
    /// The cells `text` takes as displayed from the start of the value.
    fn width(&self, text: &str) -> usize {
        text.graphemes(true)
            .fold(0, |used, g| used + self.cells(g, used))
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
        // One cell more than the text, for the cursor after its last grapheme.
        // Without it an input sized to its content scrolls its first cell away.
        let text = self.width(self.editor.text());
        ((text + 1).min(u16::MAX as usize) as u16, 1)
    }
    /// Scroll just far enough to keep the cursor in view.
    fn viewport(&mut self, size: (u16, u16), _: (u32, u32)) -> (u32, u32) {
        let width = usize::from(size.0).max(1);
        let value = self.editor.text();
        let cursor_col = self.width(&value[..self.editor.cursor()]);
        let desired = cursor_col.saturating_sub(width - 1);
        let mut left = 0;
        for g in value.graphemes(true) {
            if left >= desired {
                break;
            }
            left += self.cells(g, left);
        }
        self.editor.set_view(0, left);
        (0, 0)
    }
    fn paint(&self, canvas: &mut Canvas<'_>) {
        let width = usize::from(canvas.size().0);
        let value = self.editor.text();
        if value.is_empty() {
            canvas.text(0, 0, &self.placeholder, self.style);
        }
        // `viewport` scrolled to a grapheme boundary that keeps the cursor in view.
        let (_, left) = self.editor.view();
        let (cursor, selection) = (self.editor.cursor(), self.editor.selection());
        let mut mask = [0; 4];
        let mut cursor_x = None;
        let mut col = 0;
        for (i, g) in value.grapheme_indices(true) {
            if i == cursor {
                cursor_x = Some(col - left);
            }
            let cells = self.cells(g, col);
            if col >= left {
                if col - left >= width {
                    break;
                }
                let style = Style {
                    reverse: self.style.reverse || (canvas.focused() && selection.contains(&i)),
                    ..self.style
                };
                let shown = self.mask.map_or(g, |c| &*c.encode_utf8(&mut mask));
                cluster(canvas, (col - left) as i32, 0, shown, cells, style);
            }
            col += cells;
        }
        if canvas.focused() {
            let x = cursor_x.unwrap_or(col.saturating_sub(left));
            canvas.cursor(x as i32, 0);
            canvas.cursor_shape(self.cursor);
        }
    }
    fn event(&mut self, event: &Event) -> Response {
        if self.mask.is_some() && matches!(event, Event::Mouse(_)) {
            return Response::IGNORE;
        }
        if let (Some(limit), Some(Command::Insert(text))) = (self.limit, command(event, false)) {
            let editor = &mut self.editor;
            let count = |text: &str| text.graphemes(true).count();
            let kept = count(editor.text()) - count(&editor.text()[editor.selection()]);
            let room: String = text
                .graphemes(true)
                .take(limit.saturating_sub(kept))
                .collect();
            if room.is_empty() && editor.selection().is_empty() {
                return Response::HANDLED;
            }
            editor.insert(&room);
            return Response::CHANGED;
        }
        crate::text::edit(&mut self.editor, event, false)
    }
}
