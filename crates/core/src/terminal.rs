//! Terminal ownership and differential output, kept separate from pure drawing.
use crate::{Buffer, Color, Style};
use crossterm::{cursor, event, execute, queue, style, terminal};
use std::io::{self, IsTerminal, Write};
use std::sync::atomic::{AtomicBool, Ordering};

/// Writes changed cells to any byte sink. Does not acquire terminal modes.
#[derive(Default)]
pub struct Renderer {
    previous: Option<Buffer>,
}

impl Renderer {
    /// Forget physical contents, for example after an external terminal write.
    pub fn invalidate(&mut self) {
        self.previous = None;
    }

    /// Emit changed cells and flush once. Failed writes invalidate the shadow
    /// frame so a subsequent call repaints everything instead of losing changes.
    pub fn draw(&mut self, writer: &mut impl Write, frame: &Buffer) -> io::Result<()> {
        let previous = self.previous.take();
        let previous = previous.as_ref().filter(|old| old.area() == frame.area());
        let mut output = Vec::new();
        let mut next_position = None;
        let mut active_style = None;
        for y in 0..frame.area().height {
            for x in 0..frame.area().width {
                let cell = frame.cell(x, y).expect("frame coordinate");
                if cell.width == 0 {
                    continue;
                }
                if previous.and_then(|old| old.cell(x, y)) == Some(cell) {
                    continue;
                }
                if next_position != Some((x, y)) {
                    queue!(output, cursor::MoveTo(x, y))?;
                }
                if active_style != Some(cell.style) {
                    apply_style(&mut output, cell.style)?;
                    active_style = Some(cell.style);
                }
                output.extend_from_slice(cell.symbol.as_bytes());
                next_position = x.checked_add(cell.width as u16).map(|x| (x, y));
            }
        }
        if !output.is_empty() || previous.is_none_or(|old| old.cursor() != frame.cursor()) {
            match frame.cursor() {
                Some((x, y)) => queue!(output, cursor::MoveTo(x, y), cursor::Show)?,
                None => queue!(output, cursor::Hide)?,
            }
            queue!(
                output,
                style::SetAttribute(style::Attribute::Reset),
                style::ResetColor
            )?;
            writer.write_all(&output)?;
            writer.flush()?;
        }
        self.previous = Some(frame.clone());
        Ok(())
    }
}

/// Convert the portable palette into backend colors.
fn color(value: Color) -> style::Color {
    match value {
        Color::Default => style::Color::Reset,
        Color::Indexed(n) => style::Color::AnsiValue(n),
        Color::Rgb(r, g, b) => style::Color::Rgb { r, g, b },
    }
}

/// Replace all terminal attributes, preventing style leakage between cells.
fn apply_style(writer: &mut impl Write, value: Style) -> io::Result<()> {
    queue!(
        writer,
        style::SetAttribute(style::Attribute::Reset),
        style::SetForegroundColor(color(value.fg)),
        style::SetBackgroundColor(color(value.bg))
    )?;
    if value.bold {
        queue!(writer, style::SetAttribute(style::Attribute::Bold))?;
    }
    if value.reverse {
        queue!(writer, style::SetAttribute(style::Attribute::Reverse))?;
    }
    if value.underline {
        queue!(writer, style::SetAttribute(style::Attribute::Underlined))?;
    }
    Ok(())
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
}

impl Terminal {
    /// Acquire an interactive stdin/stdout terminal and enter the alternate screen.
    /// Existing raw sessions are rejected rather than taking over their modes.
    pub fn new() -> io::Result<Self> {
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
        };
        if terminal::is_raw_mode_enabled()? {
            return Err(io::Error::other("terminal is already in raw mode"));
        }
        terminal::enable_raw_mode()?;
        session.raw = true;
        session.screen = true;
        execute!(
            session.output,
            terminal::EnterAlternateScreen,
            cursor::Hide,
            terminal::DisableLineWrap,
            event::EnableMouseCapture,
            event::EnableBracketedPaste
        )?;
        Ok(session)
    }

    /// Current terminal dimensions; read again after resize events.
    pub fn size(&self) -> io::Result<(u16, u16)> {
        terminal::size()
    }

    /// Paint a frame whose dimensions match the current terminal.
    pub fn draw(&mut self, frame: &Buffer) -> io::Result<()> {
        self.renderer.draw(&mut self.output, frame)
    }

    /// Repaint on the next draw after another owner wrote to stdout.
    pub fn invalidate(&mut self) {
        self.renderer.invalidate();
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        if self.screen {
            let _ = execute!(
                self.output,
                style::SetAttribute(style::Attribute::Reset),
                style::ResetColor,
                event::DisableMouseCapture,
                event::DisableBracketedPaste,
                terminal::EnableLineWrap,
                cursor::Show,
                terminal::LeaveAlternateScreen
            );
        }
        if self.raw {
            let _ = terminal::disable_raw_mode();
        }
        OWNED.store(false, Ordering::Release);
    }
}

/// Terminal events are converted here so core elements never depend on crossterm.
pub fn read() -> io::Result<Option<crate::Event>> {
    use crate::{Event, Key, Modifiers, Mouse, MouseKind};
    Ok(match event::read()? {
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
                _ => return Ok(None),
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
                _ => return Ok(None),
            };
            Some(Event::Mouse(Mouse {
                x: m.column,
                y: m.row,
                kind,
            }))
        }
        _ => None,
    })
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
