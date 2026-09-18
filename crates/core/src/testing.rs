//! The same component tree and input dispatch used by an interactive terminal.
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
