//! Backend-independent input and event consumption.
mod decoder;
pub use decoder::{DecodeError, Decoder};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Modifiers {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    /// Super, Command, or Meta. Terminals report it only with `Options::keyboard`.
    pub meta: bool,
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
    Insert,
    Function(u8),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button {
    Left,
    Middle,
    Right,
}

/// `Drag` is motion with a button held; `Move` is motion with none.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseKind {
    Down(Button),
    Up(Button),
    Drag(Button),
    Move,
    ScrollUp,
    ScrollDown,
    ScrollLeft,
    ScrollRight,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mouse {
    pub x: u16,
    pub y: u16,
    pub kind: MouseKind,
    pub modifiers: Modifiers,
}
impl Mouse {
    pub fn new(x: u16, y: u16, kind: MouseKind) -> Self {
        Self {
            x,
            y,
            kind,
            modifiers: Modifiers::default(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    Key(Key, Modifiers),
    Paste(String),
    Mouse(Mouse),
    Focus,
    Blur,
    Resize(u16, u16),
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
