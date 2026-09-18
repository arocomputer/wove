//! An owned text editor shared by input elements and custom elements.
use unicode_segmentation::UnicodeSegmentation;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct State {
    text: String,
    cursor: usize,
    anchor: Option<usize>,
}

/// Positions are byte offsets on grapheme boundaries. Undo retains at most 100 edits.
#[derive(Default)]
pub struct Editor {
    state: State,
    undo: Vec<State>,
    redo: Vec<State>,
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

    fn save(&mut self) {
        if self.undo.len() == 100 {
            self.undo.remove(0);
        }
        self.undo.push(self.state.clone());
        self.redo.clear();
    }

    /// Insert at the cursor, replacing the selection as one undoable operation.
    pub fn insert(&mut self, text: &str) {
        if text.is_empty() && self.selection().is_empty() {
            return;
        }
        self.save();
        let range = self.selection();
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
    }
    fn move_to(&mut self, target: usize, extend: bool) {
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
        self.state.anchor = Some(0);
        self.state.cursor = self.text().len();
    }
    pub fn backspace(&mut self) {
        if self.selection().is_empty() {
            self.left(true);
        }
        self.insert("");
    }
    pub fn delete(&mut self) {
        if self.selection().is_empty() {
            self.right(true);
        }
        self.insert("");
    }
    pub fn undo(&mut self) {
        if let Some(state) = self.undo.pop() {
            self.redo.push(std::mem::replace(&mut self.state, state));
        }
    }
    pub fn redo(&mut self) {
        if let Some(state) = self.redo.pop() {
            self.undo.push(std::mem::replace(&mut self.state, state));
        }
    }
}
