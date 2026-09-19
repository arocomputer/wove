//! Ownership of the local terminal, kept separate from pure drawing.
use crate::{Buffer, Depth, Inline, Renderer};
pub use crate::{Options, ScreenMode};
pub use crossterm::event::EventStream;
use crossterm::{event, terminal};
use std::io::{self, IsTerminal, Write};
use std::sync::atomic::{AtomicBool, Ordering};

mod query;

static OWNED: AtomicBool = AtomicBool::new(false);

/// Owns the terminal session. Drop restores modes on normal return and panic
/// unwinding. Process aborts and uncatchable signals cannot run Drop.
/// Only one session may exist; callers must not change terminal modes behind it.
pub struct Terminal {
    output: io::Stdout,
    options: Options,
    depth: Depth,
    renderer: Renderer,
    inline: Option<Inline>,
    raw: bool,
    entered: bool,
}

impl Terminal {
    /// Acquire an interactive stdin/stdout terminal and enter the alternate screen.
    /// Existing raw sessions are rejected rather than taking over their modes.
    pub fn new() -> io::Result<Self> {
        Self::with_options(Options::default())
    }

    pub fn with_mode(screen: ScreenMode) -> io::Result<Self> {
        Self::with_options(Options {
            screen,
            ..Options::default()
        })
    }

    /// Colors are mapped to what the environment says the terminal can show.
    pub fn with_options(options: Options) -> io::Result<Self> {
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
        let depth = Depth::detect();
        let mut session = Self {
            output: io::stdout(),
            options,
            depth,
            renderer: Renderer::with_depth(depth),
            inline: None,
            raw: false,
            entered: false,
        };
        if terminal::is_raw_mode_enabled()? {
            return Err(io::Error::other("terminal is already in raw mode"));
        }
        session.resume()?;
        Ok(session)
    }

    /// Restore terminal modes temporarily while retaining exclusive ownership.
    /// Resume before drawing again. This permits an application to run a child
    /// UI. An inline session first parks the cursor beneath its last frame.
    pub fn suspend(&mut self) -> io::Result<()> {
        let mut result = Ok(());
        if self.entered {
            if self.options.screen == ScreenMode::Inline {
                if let Some(inline) = &mut self.inline {
                    result = inline.finish(&mut self.output);
                }
            }
            result = result.and(self.options.leave(&mut self.output));
            self.entered = result.is_err();
        }
        if self.raw {
            let raw = terminal::disable_raw_mode();
            self.raw = raw.is_err();
            result = result.and(raw);
        }
        self.invalidate();
        result
    }

    /// Reacquire modes after suspension and force a complete repaint.
    pub fn resume(&mut self) -> io::Result<()> {
        if self.raw && self.entered {
            return Ok(());
        }
        if !self.raw {
            terminal::enable_raw_mode()?;
            self.raw = true;
        }
        // Raw escape sequences need virtual terminal processing on Windows.
        #[cfg(windows)]
        let _ = crossterm::ansi_support::supports_ansi();
        self.entered = true;
        self.renderer.invalidate();
        match &mut self.inline {
            // Whatever ran during the suspension may have written anywhere.
            Some(inline) => inline.invalidate(),
            None if self.options.screen == ScreenMode::Inline => self.anchor()?,
            None => {}
        }
        self.options.enter(&mut self.output)
    }

    /// Start the inline region on a fresh line at the cursor. A terminal that
    /// does not report its cursor gets a cleared screen instead.
    fn anchor(&mut self) -> io::Result<()> {
        let cursor = query::cursor(&mut self.output)?;
        let row = match cursor {
            Some((column, row)) if column > 0 => {
                self.output.write_all(b"\r\n")?;
                row.saturating_add(1)
            }
            Some((_, row)) => row,
            None => 0,
        };
        let mut inline = Inline::new(row, self.depth);
        if cursor.is_none() {
            inline.invalidate();
        }
        self.inline = Some(inline);
        Ok(())
    }

    /// Move between screens while running, for example from an inline session
    /// to a full-screen view and back. The main screen keeps what the inline
    /// session drew while the alternate screen is up.
    pub fn switch(&mut self, screen: ScreenMode) -> io::Result<()> {
        let old = self.options.screen;
        if old == screen {
            return Ok(());
        }
        self.options.screen = screen;
        self.renderer.invalidate();
        if !self.entered {
            return Ok(());
        }
        if old == ScreenMode::Alternate {
            self.output.write_all(b"\x1b[?1049l")?;
        }
        if screen == ScreenMode::Alternate {
            self.output.write_all(b"\x1b[?1049h")?;
        }
        self.output.flush()?;
        match (&mut self.inline, screen, old) {
            (None, ScreenMode::Inline, _) => self.anchor()?,
            (Some(inline), ScreenMode::Inline, ScreenMode::Main) => inline.invalidate(),
            _ => {}
        }
        Ok(())
    }

    /// Current terminal dimensions; read again after resize events.
    pub fn size(&self) -> io::Result<(u16, u16)> {
        terminal::size()
    }

    /// Paint a frame. A full-screen frame matches the terminal's dimensions.
    /// An inline frame matches its width and may be of any height.
    pub fn draw(&mut self, frame: &Buffer) -> io::Result<()> {
        if !self.raw || !self.entered {
            return Err(io::Error::other("terminal is suspended"));
        }
        match (&mut self.inline, self.options.screen) {
            (Some(inline), ScreenMode::Inline) => {
                inline.draw(&mut self.output, frame, terminal::size()?.1)
            }
            _ => self.renderer.draw(&mut self.output, frame),
        }
    }

    /// Inline only: release the first `rows` rows of the last frame to the
    /// terminal's history. See `Inline::commit`.
    pub fn commit(&mut self, rows: u16) {
        if let Some(inline) = &mut self.inline {
            inline.commit(rows);
        }
    }

    /// Repaint on the next draw after another owner wrote to stdout.
    pub fn invalidate(&mut self) {
        self.renderer.invalidate();
        if let Some(inline) = &mut self.inline {
            inline.invalidate();
        }
    }

    /// Ask the terminal for its background color, to choose between light and
    /// dark palettes. Call this before reading events: the reply arrives on
    /// stdin, and keys typed during the short wait are lost. `None` means the
    /// terminal did not answer.
    pub fn background(&mut self) -> io::Result<Option<(u8, u8, u8)>> {
        query::background(&mut self.output)
    }
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
    use crate::{Button, Event, Key, Modifiers, Mouse, MouseKind};
    let modifiers = |m: event::KeyModifiers| Modifiers {
        ctrl: m.contains(event::KeyModifiers::CONTROL),
        alt: m.contains(event::KeyModifiers::ALT),
        shift: m.contains(event::KeyModifiers::SHIFT),
        meta: m.intersects(
            event::KeyModifiers::SUPER | event::KeyModifiers::META | event::KeyModifiers::HYPER,
        ),
    };
    match input {
        event::Event::Key(key) if key.kind != event::KeyEventKind::Release => {
            let mut mods = modifiers(key.modifiers);
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
            let button = |button| match button {
                event::MouseButton::Left => Button::Left,
                event::MouseButton::Middle => Button::Middle,
                event::MouseButton::Right => Button::Right,
            };
            let kind = match m.kind {
                event::MouseEventKind::Down(b) => MouseKind::Down(button(b)),
                event::MouseEventKind::Up(b) => MouseKind::Up(button(b)),
                event::MouseEventKind::Drag(b) => MouseKind::Drag(button(b)),
                event::MouseEventKind::Moved => MouseKind::Move,
                event::MouseEventKind::ScrollUp => MouseKind::ScrollUp,
                event::MouseEventKind::ScrollDown => MouseKind::ScrollDown,
                event::MouseEventKind::ScrollLeft => MouseKind::ScrollLeft,
                event::MouseEventKind::ScrollRight => MouseKind::ScrollRight,
            };
            Some(Event::Mouse(Mouse {
                x: m.column,
                y: m.row,
                kind,
                modifiers: modifiers(m.modifiers),
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

/// Run a tree on the alternate screen, calling `update` after each dispatched
/// event. The loop ends when `update` returns false; which keys quit is the
/// application's decision, so handle one or the terminal stays captured.
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
        let result = tree.dispatch(event.clone()).map_err(io::Error::other)?;
        if !update(tree, &event, &result) {
            break;
        }
    }
    Ok(())
}
