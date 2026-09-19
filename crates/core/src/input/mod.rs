//! Backend-independent input and event consumption.
mod decoder;
pub use decoder::Decoder;

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
    /// The node gained or lost the tree's focus.
    Focus,
    Blur,
    /// The pointer entered or left the node. Needs `Options::motion`.
    Enter,
    Leave,
    /// The terminal window gained or lost focus. Nodes stay focused.
    WindowFocus(bool),
    Resize(u16, u16),
}

impl Event {
    /// A key event in the one form every input source produces. A character
    /// already reflects Shift, so `Char` keys never carry it: Shift+a is
    /// `Char('A')`, and Ctrl+Shift+a is `Char('A')` with `ctrl`. Bindings then
    /// match the same way on a local terminal, over SSH, and with or without
    /// kitty key reports.
    pub fn key(key: Key, mut modifiers: Modifiers) -> Self {
        let key = match key {
            Key::Char(c) => {
                let mut upper = c.to_uppercase();
                let shifted = match (modifiers.shift, upper.next(), upper.next()) {
                    (true, Some(upper), None) => upper,
                    _ => c,
                };
                modifiers.shift = false;
                Key::Char(shifted)
            }
            key => key,
        };
        Self::Key(key, modifiers)
    }
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
