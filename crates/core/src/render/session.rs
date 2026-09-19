//! The terminal modes a session enables, as bytes any transport can carry.
use std::io::{self, Write};

/// Where frames are drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ScreenMode {
    /// A full-screen buffer that leaves the shell's screen untouched on exit.
    #[default]
    Alternate,
    /// Full-screen drawing that overwrites the visible main screen.
    Main,
    /// Frames grow downward from the launch cursor on the main screen. Rows
    /// that scroll off the top stay in the terminal's own scrollback.
    Inline,
}

/// The modes a session turns on when it starts and off again when it ends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Options {
    pub screen: ScreenMode,
    /// Report clicks, drags, motion, and the wheel. Turn this off to leave text
    /// selection to the terminal.
    pub mouse: bool,
    /// Deliver pasted text as one event instead of as typed keys.
    pub paste: bool,
    /// Report when the terminal window gains or loses focus.
    pub focus: bool,
    /// Ask for unambiguous key reports (the kitty keyboard protocol), which
    /// distinguish keys such as Shift+Enter. Terminals without it ignore the request.
    pub keyboard: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            screen: ScreenMode::default(),
            mouse: true,
            paste: true,
            focus: true,
            keyboard: false,
        }
    }
}

impl Options {
    /// Enable input reporting and screen ownership.
    pub fn enter(&self, out: &mut impl Write) -> io::Result<()> {
        if self.screen == ScreenMode::Alternate {
            out.write_all(b"\x1b[?1049h")?;
        }
        out.write_all(b"\x1b[?25l\x1b[?7l")?;
        if self.mouse {
            out.write_all(b"\x1b[?1000h\x1b[?1002h\x1b[?1003h\x1b[?1006h")?;
        }
        if self.paste {
            out.write_all(b"\x1b[?2004h")?;
        }
        if self.focus {
            out.write_all(b"\x1b[?1004h")?;
        }
        if self.keyboard {
            out.write_all(b"\x1b[>1u")?;
        }
        out.flush()
    }

    /// Undo only the modes `enter` enabled.
    pub fn leave(&self, out: &mut impl Write) -> io::Result<()> {
        out.write_all(b"\x1b[0m")?;
        if self.keyboard {
            out.write_all(b"\x1b[<1u")?;
        }
        if self.focus {
            out.write_all(b"\x1b[?1004l")?;
        }
        if self.paste {
            out.write_all(b"\x1b[?2004l")?;
        }
        if self.mouse {
            out.write_all(b"\x1b[?1006l\x1b[?1003l\x1b[?1002l\x1b[?1000l")?;
        }
        out.write_all(b"\x1b[?7h\x1b[?25h")?;
        if self.screen == ScreenMode::Alternate {
            out.write_all(b"\x1b[?1049l")?;
        }
        out.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(options: Options) -> String {
        let mut bytes = Vec::new();
        options.enter(&mut bytes).unwrap();
        options.leave(&mut bytes).unwrap();
        String::from_utf8(bytes).unwrap()
    }

    #[test]
    fn only_the_alternate_screen_switches_buffers_and_every_mode_is_undone() {
        for screen in [ScreenMode::Main, ScreenMode::Inline, ScreenMode::Alternate] {
            let output = session(Options {
                screen,
                keyboard: true,
                ..Options::default()
            });
            let alternate = screen == ScreenMode::Alternate;
            assert_eq!(output.contains("\x1b[?1049h"), alternate);
            assert_eq!(output.contains("\x1b[?1049l"), alternate);
            for undo in [
                "\x1b[?2004l",
                "\x1b[?1004l",
                "\x1b[?1000l",
                "\x1b[<1u",
                "\x1b[?25h",
            ] {
                assert!(output.contains(undo), "{undo:?}");
            }
        }
    }

    #[test]
    fn modes_an_application_declines_are_never_touched() {
        let output = session(Options {
            mouse: false,
            focus: false,
            ..Options::default()
        });
        assert!(!output.contains("?1000") && !output.contains("?1004"));
        assert!(!output.contains('u'), "keyboard reporting is opt-in");
    }
}
