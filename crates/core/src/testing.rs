//! The same element tree and input dispatch used by an interactive terminal.
use crate::{Buffer, Dispatch, Error, Event, Tree};

pub struct Screen {
    pub tree: Tree,
    width: u16,
    height: u16,
}
impl Screen {
    pub fn new(width: u16, height: u16) -> Self {
        Self {
            tree: Tree::new(),
            width,
            height,
        }
    }
    pub fn resize(&mut self, width: u16, height: u16) {
        self.width = width;
        self.height = height;
    }
    pub fn frame(&mut self) -> Result<&Buffer, Error> {
        self.tree.frame(self.width, self.height)
    }
    pub fn send(&mut self, event: impl Into<Event>) -> Result<Dispatch, Error> {
        self.frame()?;
        self.tree.dispatch(event.into())
    }
}

/// A clock advanced explicitly by tests, with no sleeping or wall-clock reads.
#[derive(Default)]
pub struct Clock {
    now: std::time::Duration,
}
impl Clock {
    pub fn now(&self) -> std::time::Duration {
        self.now
    }
    pub fn advance(&mut self, elapsed: std::time::Duration) {
        self.now = self.now.saturating_add(elapsed);
    }
}

/// Retains only changed frames, including their styles and cursor positions.
#[derive(Default)]
pub struct Recorder {
    frames: Vec<(std::time::Duration, Buffer)>,
}
impl Recorder {
    pub fn record(&mut self, at: std::time::Duration, frame: &Buffer) {
        if self
            .frames
            .last()
            .is_none_or(|(_, previous)| previous != frame)
        {
            self.frames.push((at, frame.clone()));
        }
    }
    pub fn frames(&self) -> &[(std::time::Duration, Buffer)] {
        &self.frames
    }
    pub fn clear(&mut self) {
        self.frames.clear();
    }
}
