//! Backend-independent input and event consumption.

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Modifiers {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Char(char),
    Enter,
    Escape,
    Tab,
    Backspace,
    Delete,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    PageUp,
    PageDown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseKind {
    Down,
    Up,
    Move,
    ScrollUp,
    ScrollDown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mouse {
    pub x: u16,
    pub y: u16,
    pub kind: MouseKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    Key(Key, Modifiers),
    Paste(String),
    Mouse(Mouse),
    Focus,
    Blur,
}

impl From<Key> for Event {
    fn from(key: Key) -> Self {
        Self::Key(key, Modifiers::default())
    }
}

/// A handler can consume an event without repainting, or repaint and let it bubble.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Response {
    pub handled: bool,
    pub changed: bool,
}

impl Response {
    pub const IGNORE: Self = Self {
        handled: false,
        changed: false,
    };
    pub const HANDLED: Self = Self {
        handled: true,
        changed: false,
    };
    pub const CHANGED: Self = Self {
        handled: true,
        changed: true,
    };
}
