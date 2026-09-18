//! Terminal ownership and differential output, kept separate from pure drawing.
use crate::Buffer;
pub use crossterm::event::EventStream;
use crossterm::{cursor, event, execute, style, terminal};
use std::io::{self, IsTerminal};
use std::sync::atomic::{AtomicBool, Ordering};

mod renderer;
pub use renderer::Renderer;

/// Choose a full-screen buffer. Main screen drawing overwrites visible terminal
/// cells; it is not an inline prompt or an append-only scrollback interface.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ScreenMode {
    #[default]
    Alternate,
    Main,
}

static OWNED: AtomicBool = AtomicBool::new(false);

/// Owns a full-screen terminal session. Drop restores modes on normal return
/// and panic unwinding. Process aborts and uncatchable signals cannot run Drop.
/// Only one session may exist; callers must not change terminal modes behind it.
pub struct Terminal {
    output: io::Stdout,
    renderer: Renderer,
    raw: bool,
    screen: bool,
    mode: ScreenMode,
}

impl Terminal {
    /// Acquire an interactive stdin/stdout terminal and enter the alternate screen.
    /// Existing raw sessions are rejected rather than taking over their modes.
    pub fn new() -> io::Result<Self> {
        Self::with_mode(ScreenMode::Alternate)
    }

    pub fn with_mode(mode: ScreenMode) -> io::Result<Self> {
        if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
            return Err(io::Error::other(
                "wove needs an interactive stdin and stdout",
            ));
        }
        if OWNED
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Err(io::Error::other("a wove terminal is already active"));
        }
        let mut session = Self {
            output: io::stdout(),
            renderer: Renderer::default(),
            raw: false,
            screen: false,
            mode,
        };
        if terminal::is_raw_mode_enabled()? {
            return Err(io::Error::other("terminal is already in raw mode"));
        }
        session.resume()?;
        Ok(session)
    }

    /// Restore terminal modes temporarily while retaining exclusive ownership.
    /// Resume before drawing again. This permits an application to run a child UI.
    pub fn suspend(&mut self) -> io::Result<()> {
        let screen_result = if self.screen {
            let result = leave(&mut self.output, self.mode);
            if result.is_ok() {
                self.screen = false;
            }
            result
        } else {
            Ok(())
        };
        let raw_result = if self.raw {
            let result = terminal::disable_raw_mode();
            if result.is_ok() {
                self.raw = false;
            }
            result
        } else {
            Ok(())
        };
        self.renderer.invalidate();
        screen_result.and(raw_result)
    }
    /// Reacquire modes after suspension and force a complete repaint.
    pub fn resume(&mut self) -> io::Result<()> {
        if self.raw && self.screen {
            return Ok(());
        }
        if !self.raw {
            terminal::enable_raw_mode()?;
            self.raw = true;
        }
        self.screen = true;
        enter(&mut self.output, self.mode)?;
        self.renderer.invalidate();
        Ok(())
    }

    /// Current terminal dimensions; read again after resize events.
    pub fn size(&self) -> io::Result<(u16, u16)> {
        terminal::size()
    }

    /// Paint a frame whose dimensions match the current terminal.
    pub fn draw(&mut self, frame: &Buffer) -> io::Result<()> {
        if !self.raw || !self.screen {
            return Err(io::Error::other("terminal is suspended"));
        }
        self.renderer.draw(&mut self.output, frame)
    }

    /// Repaint on the next draw after another owner wrote to stdout.
    pub fn invalidate(&mut self) {
        self.renderer.invalidate();
    }
}

/// Enable input reporting and screen ownership as one output transaction.
fn enter(output: &mut impl std::io::Write, mode: ScreenMode) -> io::Result<()> {
    if mode == ScreenMode::Alternate {
        execute!(output, terminal::EnterAlternateScreen)?;
    }
    execute!(
        output,
        cursor::Hide,
        terminal::DisableLineWrap,
        event::EnableMouseCapture,
        event::EnableBracketedPaste,
        event::EnableFocusChange
    )
}

/// Undo only the terminal modes this session enabled.
fn leave(output: &mut impl std::io::Write, mode: ScreenMode) -> io::Result<()> {
    execute!(
        output,
        style::SetAttribute(style::Attribute::Reset),
        style::ResetColor,
        event::DisableMouseCapture,
        event::DisableBracketedPaste,
        event::DisableFocusChange,
        terminal::EnableLineWrap,
        cursor::Show
    )?;
    if mode == ScreenMode::Alternate {
        execute!(output, terminal::LeaveAlternateScreen)?;
    }
    Ok(())
}
impl Drop for Terminal {
    fn drop(&mut self) {
        let _ = self.suspend();
        OWNED.store(false, Ordering::Release);
    }
}

/// Terminal events are converted here so core elements never depend on crossterm.
pub fn read() -> io::Result<Option<crate::Event>> {
    Ok(convert(event::read()?))
}

/// Convert backend input for synchronous readers and asynchronous event streams.
pub fn convert(input: event::Event) -> Option<crate::Event> {
    use crate::{Event, Key, Modifiers, Mouse, MouseKind};
    match input {
        event::Event::Key(key) if key.kind != event::KeyEventKind::Release => {
            let mut mods = Modifiers {
                ctrl: key.modifiers.contains(event::KeyModifiers::CONTROL),
                alt: key.modifiers.contains(event::KeyModifiers::ALT),
                shift: key.modifiers.contains(event::KeyModifiers::SHIFT),
            };
            let key = match key.code {
                event::KeyCode::Char(c) => Key::Char(c),
                event::KeyCode::Enter => Key::Enter,
                event::KeyCode::Esc => Key::Escape,
                event::KeyCode::Tab => Key::Tab,
                event::KeyCode::BackTab => {
                    mods.shift = true;
                    Key::Tab
                }
                event::KeyCode::Backspace => Key::Backspace,
                event::KeyCode::Delete => Key::Delete,
                event::KeyCode::Left => Key::Left,
                event::KeyCode::Right => Key::Right,
                event::KeyCode::Up => Key::Up,
                event::KeyCode::Down => Key::Down,
                event::KeyCode::Home => Key::Home,
                event::KeyCode::End => Key::End,
                event::KeyCode::PageUp => Key::PageUp,
                event::KeyCode::PageDown => Key::PageDown,
                event::KeyCode::Insert => Key::Insert,
                event::KeyCode::F(number) => Key::Function(number),
                _ => return None,
            };
            Some(Event::Key(key, mods))
        }
        event::Event::Paste(s) => Some(Event::Paste(s)),
        event::Event::Mouse(m) => {
            let kind = match m.kind {
                event::MouseEventKind::Down(event::MouseButton::Left) => MouseKind::Down,
                event::MouseEventKind::Up(event::MouseButton::Left) => MouseKind::Up,
                event::MouseEventKind::Moved
                | event::MouseEventKind::Drag(event::MouseButton::Left) => MouseKind::Move,
                event::MouseEventKind::ScrollUp => MouseKind::ScrollUp,
                event::MouseEventKind::ScrollDown => MouseKind::ScrollDown,
                _ => return None,
            };
            Some(Event::Mouse(Mouse {
                x: m.column,
                y: m.row,
                kind,
            }))
        }
        event::Event::Resize(width, height) => Some(Event::Resize(width, height)),
        event::Event::FocusGained => Some(Event::Focus),
        event::Event::FocusLost => Some(Event::Blur),
        _ => None,
    }
}

/// Wait without reading. Custom loops can multiplex terminal input and their own work.
pub fn poll(timeout: std::time::Duration) -> io::Result<bool> {
    event::poll(timeout)
}

/// Run a tree with an application callback after each dispatched event. Escape
/// and Ctrl-C exit; the callback can return false to finish for another reason.
pub fn run(
    tree: &mut crate::Tree,
    mut update: impl FnMut(&mut crate::Tree, &crate::Event, &crate::Dispatch) -> bool,
) -> io::Result<()> {
    let mut terminal = Terminal::new()?;
    loop {
        let (w, h) = terminal.size()?;
        terminal.draw(tree.frame(w, h).map_err(io::Error::other)?)?;
        let Some(event) = read()? else {
            continue;
        };
        if matches!(
            &event,
            crate::Event::Key(crate::Key::Escape, _)
                | crate::Event::Key(crate::Key::Char('c'), crate::Modifiers { ctrl: true, .. })
        ) {
            break;
        }
        let result = tree.dispatch(event.clone()).map_err(io::Error::other)?;
        if !update(tree, &event, &result) {
            break;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn main_screen_never_switches_buffers_and_both_modes_restore_input() {
        for mode in [ScreenMode::Main, ScreenMode::Alternate] {
            let mut bytes = Vec::new();
            enter(&mut bytes, mode).unwrap();
            leave(&mut bytes, mode).unwrap();
            let output = String::from_utf8(bytes).unwrap();
            assert_eq!(
                output.contains("\x1b[?1049h"),
                mode == ScreenMode::Alternate
            );
            assert_eq!(
                output.contains("\x1b[?1049l"),
                mode == ScreenMode::Alternate
            );
            assert!(output.contains("\x1b[?2004l"));
            assert!(output.contains("\x1b[?1004l"));
        }
    }
}
