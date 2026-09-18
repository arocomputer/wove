//! An owned text editor shared by input elements and custom elements.
use std::collections::VecDeque;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct State {
    text: String,
    cursor: usize,
    anchor: Option<usize>,
}

/// An edit stores only the replaced text and selection endpoints.
struct Edit {
    start: usize,
    removed: String,
    inserted: String,
    before: (usize, Option<usize>),
    after: (usize, Option<usize>),
}

/// Positions are byte offsets on grapheme boundaries. Undo retains at most 100 edits.
#[derive(Default)]
pub struct Editor {
    state: State,
    undo: VecDeque<Edit>,
    redo: Vec<Edit>,
    column: Option<usize>,
}

impl Editor {
    pub fn new(text: impl Into<String>) -> Self {
        let text = text.into();
        Self {
            state: State {
                cursor: text.len(),
                text,
                anchor: None,
            },
            ..Self::default()
        }
    }
    pub fn text(&self) -> &str {
        &self.state.text
    }
    pub fn cursor(&self) -> usize {
        self.state.cursor
    }
    pub fn selection(&self) -> std::ops::Range<usize> {
        let a = self.state.anchor.unwrap_or(self.state.cursor);
        a.min(self.state.cursor)..a.max(self.state.cursor)
    }
    pub fn set(&mut self, text: impl Into<String>) {
        *self = Self::new(text);
    }

    /// Insert at the cursor, replacing the selection as one undoable operation.
    pub fn insert(&mut self, text: &str) {
        if text.is_empty() && self.selection().is_empty() {
            return;
        }
        let range = self.selection();
        let before = (self.state.cursor, self.state.anchor);
        let removed = self.state.text[range.clone()].to_owned();
        self.state.text.replace_range(range.clone(), text);
        let end = range.start + text.len();
        // Insertion can join adjacent graphemes; advance to the next valid boundary.
        self.state.cursor = self
            .state
            .text
            .grapheme_indices(true)
            .map(|(i, _)| i)
            .find(|i| *i >= end)
            .unwrap_or(self.state.text.len());
        self.state.anchor = None;
        self.column = None;
        if self.undo.len() == 100 {
            self.undo.pop_front();
        }
        self.undo.push_back(Edit {
            start: range.start,
            removed,
            inserted: text.to_owned(),
            before,
            after: (self.state.cursor, self.state.anchor),
        });
        self.redo.clear();
    }
    fn move_to(&mut self, target: usize, extend: bool) {
        self.column = None;
        if extend {
            self.state.anchor.get_or_insert(self.state.cursor);
        } else {
            self.state.anchor = None;
        }
        self.state.cursor = target;
    }
    pub fn left(&mut self, extend: bool) {
        if !extend && !self.selection().is_empty() {
            self.move_to(self.selection().start, false);
            return;
        }
        let target = self.text()[..self.cursor()]
            .grapheme_indices(true)
            .next_back()
            .map_or(0, |(i, _)| i);
        self.move_to(target, extend);
    }
    pub fn right(&mut self, extend: bool) {
        if !extend && !self.selection().is_empty() {
            self.move_to(self.selection().end, false);
            return;
        }
        let n = self.text()[self.cursor()..]
            .graphemes(true)
            .next()
            .map_or(0, str::len);
        self.move_to(self.cursor() + n, extend);
    }
    pub fn home(&mut self, extend: bool) {
        self.move_to(0, extend);
    }
    pub fn end(&mut self, extend: bool) {
        self.move_to(self.text().len(), extend);
    }
    pub fn select_all(&mut self) {
        self.column = None;
        self.state.anchor = Some(0);
        self.state.cursor = self.text().len();
    }
    pub fn backspace(&mut self) {
        self.erase(true);
    }
    pub fn delete(&mut self) {
        self.erase(false);
    }
    /// Keep the user's selection separate from the range selected for deletion.
    fn erase(&mut self, backward: bool) {
        let before = (self.state.cursor, self.state.anchor);
        if self.selection().is_empty() {
            if backward {
                self.left(true);
            } else {
                self.right(true);
            }
        }
        if self.selection().is_empty() {
            (self.state.cursor, self.state.anchor) = before;
            return;
        }
        self.insert("");
        self.undo.back_mut().expect("deletion recorded").before = before;
    }
    /// Move to a byte offset, snapping backward to a grapheme boundary.
    pub fn seek(&mut self, offset: usize, extend: bool) {
        let target = if offset >= self.text().len() {
            self.text().len()
        } else {
            self.text()
                .grapheme_indices(true)
                .map(|(i, _)| i)
                .take_while(|i| *i <= offset)
                .last()
                .unwrap_or(0)
        };
        self.move_to(target, extend);
    }
    /// Move within logical lines while preserving the preferred display column.
    pub fn vertical(&mut self, delta: isize, extend: bool) {
        let starts: Vec<usize> = std::iter::once(0)
            .chain(self.text().match_indices('\n').map(|(i, _)| i + 1))
            .collect();
        let row = starts.partition_point(|i| *i <= self.cursor()) - 1;
        let column = self
            .column
            .unwrap_or_else(|| self.text()[starts[row]..self.cursor()].width());
        let next = row.saturating_add_signed(delta).min(starts.len() - 1);
        let line = self.text()[starts[next]..].split('\n').next().unwrap_or("");
        let mut width = 0;
        let mut offset = starts[next];
        for g in line.graphemes(true) {
            if width + g.width() > column {
                break;
            }
            width += g.width();
            offset += g.len();
        }
        self.seek(offset, extend);
        self.column = Some(column);
    }
    /// Move to the beginning of the current logical line.
    pub fn line_home(&mut self, extend: bool) {
        let start = self.text()[..self.cursor()]
            .rfind('\n')
            .map_or(0, |i| i + 1);
        self.move_to(start, extend);
    }
    /// Move to the end of the current logical line.
    pub fn line_end(&mut self, extend: bool) {
        let end = self.text()[self.cursor()..]
            .find('\n')
            .map_or(self.text().len(), |i| self.cursor() + i);
        self.seek(end, extend);
    }
    pub fn undo(&mut self) {
        if let Some(edit) = self.undo.pop_back() {
            self.state
                .text
                .replace_range(edit.start..edit.start + edit.inserted.len(), &edit.removed);
            (self.state.cursor, self.state.anchor) = edit.before;
            self.column = None;
            self.redo.push(edit);
        }
    }
    pub fn redo(&mut self) {
        if let Some(edit) = self.redo.pop() {
            self.state
                .text
                .replace_range(edit.start..edit.start + edit.removed.len(), &edit.inserted);
            (self.state.cursor, self.state.anchor) = edit.after;
            self.column = None;
            self.undo.push_back(edit);
        }
    }
}
