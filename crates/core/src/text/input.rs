//! Editing commands shared by single-line and multiline inputs.
use super::Editor;
use crate::{Event, Key, Response};

/// Apply an editing event. Multiline inputs accept newlines and logical line motion.
pub(crate) fn edit(editor: &mut Editor, event: &Event, multiline: bool) -> Response {
    match event {
        Event::Paste(value) => editor.insert(&clean(value, multiline)),
        Event::Key(key, m) => match key {
            Key::Char('a') if m.ctrl => editor.select_all(),
            Key::Char('z') if m.ctrl && m.shift => editor.redo(),
            Key::Char('z') if m.ctrl => editor.undo(),
            Key::Char('y') if m.ctrl => editor.redo(),
            Key::Char(c) if !m.ctrl && !m.alt && !c.is_control() => editor.insert(&c.to_string()),
            Key::Enter if multiline => editor.insert("\n"),
            Key::Up if multiline => editor.vertical(-1, m.shift),
            Key::Down if multiline => editor.vertical(1, m.shift),
            Key::Home if multiline && !m.ctrl => editor.line_home(m.shift),
            Key::End if multiline && !m.ctrl => editor.line_end(m.shift),
            Key::Left => editor.left(m.shift),
            Key::Right => editor.right(m.shift),
            Key::Home => editor.home(m.shift),
            Key::End => editor.end(m.shift),
            Key::Backspace => editor.backspace(),
            Key::Delete => editor.delete(),
            _ => return Response::IGNORE,
        },
        _ => return Response::IGNORE,
    }
    Response::CHANGED
}

/// Normalize pasted newlines and remove terminal controls before storing input.
pub(crate) fn clean(value: &str, multiline: bool) -> String {
    value
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .chars()
        .filter(|c| !c.is_control() || (multiline && *c == '\n'))
        .collect()
}
