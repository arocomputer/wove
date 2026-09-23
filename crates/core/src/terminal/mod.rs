//! Ownership of the local terminal, kept separate from pure drawing.
use crate::render::{self, Park, Progress, POP_TITLE, PUSH_TITLE};
use crate::{Buffer, Depth, Inline, Renderer};
pub use crate::{Options, ScreenMode};
pub use crossterm::event::EventStream;
use crossterm::{event, terminal};
use std::io::{self, IsTerminal, Write};
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Mutex, Once};

mod query;
pub use query::Capabilities;

static OWNED: AtomicBool = AtomicBool::new(false);
/// What to undo if the process ends without dropping the session.
struct Active {
    /// Raw mode is already owned; screen and input modes may not be entered yet.
    options: Option<Options>,
    /// Puts the cursor beneath an inline frame, so that what prints next
    /// lands under the frame instead of over it.
    park: Park,
    surface: Surface,
}

impl Active {
    /// Undo everything the session changed, in the order that leaves the
    /// cursor beneath the frame and the user's title in place.
    fn leave(self, output: &mut impl Write) -> io::Result<()> {
        self.park.write(output)?;
        self.surface.restore(output)?;
        match self.options {
            Some(options) => options.leave(output),
            None => output.flush(),
        }
    }
}

/// What the application showed outside the frame that ending the session
/// must take back: a title pushed over the user's, and taskbar progress.
#[derive(Clone, Copy, Default)]
struct Surface {
    title: bool,
    progress: bool,
    tmux: bool,
}

impl Surface {
    fn restore(self, output: &mut impl Write) -> io::Result<()> {
        if self.progress {
            output.write_all(&render::progress(Progress::Clear, self.tmux))?;
        }
        if self.title {
            output.write_all(POP_TITLE)?;
        }
        Ok(())
    }
}

/// Whoever takes this, the session, the panic hook, or the signal thread,
/// restores the terminal, so it happens exactly once.
static ACTIVE: Mutex<Option<Active>> = Mutex::new(None);

/// A panic elsewhere must not stop the terminal from being restored, so a
/// poisoned lock is used like any other.
fn active() -> std::sync::MutexGuard<'static, Option<Active>> {
    ACTIVE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Restore the terminal from wherever the process is ending.
fn rescue() {
    let taken = active().take();
    if let Some(active) = taken {
        let _ = active.leave(&mut io::stdout());
        let _ = terminal::disable_raw_mode();
    }
}

/// What the signal thread does when a fatal signal arrives.
static SIGNALS: AtomicU8 = AtomicU8::new(DEFAULT);
/// No session is active: the signal has its usual effect.
const DEFAULT: u8 = 0;
/// A session asked for rescue: restore the terminal, then the usual effect.
const RESCUE: u8 = 1;
/// A session handles signals itself: stay out of the way.
const HANDS_OFF: u8 = 2;

/// A panic message printed onto the alternate screen vanishes with it, and one
/// printed in raw mode staircases. Restore first, then let the message print.
fn install_panic_hook() {
    static HOOK: Once = Once::new();
    HOOK.call_once(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            rescue();
            previous(info);
        }));
    });
}

/// A killed process runs no destructors, which would leave the user's shell
/// in raw mode with mouse reports pouring in. Restore, then let the signal
/// have its usual effect.
///
/// Registering for a signal replaces its default action for the life of the
/// process, so the thread stays once started and consults `SIGNALS`: it still
/// ends the process when no session is active, and does nothing while a
/// session that handles signals itself is.
#[cfg(unix)]
fn watch_signals() {
    use signal_hook::consts::{SIGHUP, SIGINT, SIGQUIT, SIGTERM};
    static WATCH: Once = Once::new();
    WATCH.call_once(|| {
        let Ok(mut signals) =
            signal_hook::iterator::Signals::new([SIGHUP, SIGINT, SIGQUIT, SIGTERM])
        else {
            return;
        };
        std::thread::spawn(move || {
            for signal in signals.forever() {
                if SIGNALS.load(Ordering::Acquire) == HANDS_OFF {
                    continue;
                }
                rescue();
                let _ = signal_hook::low_level::emulate_default_handler(signal);
            }
        });
    });
}
#[cfg(not(unix))]
fn watch_signals() {}

/// Owns the terminal session. Drop restores modes on normal return; a panic
/// and, unless `Options::signals` is off, a fatal signal restore them too.
/// Only an abort or an uncatchable signal leaves them behind. Only one session
/// may exist; callers must not change terminal modes behind it.
///
/// A session is `Send`. An application whose terminal can be slow, over SSH
/// or inside a multiplexer, can draw from a thread of its own and hand it
/// cloned frames, so a blocked write never stalls input handling.
pub struct Terminal {
    output: io::Stdout,
    options: Options,
    depth: Depth,
    renderer: Renderer,
    inline: Option<Inline>,
    capabilities: Capabilities,
    /// Where the cursor was when the session started, until an inline frame
    /// is anchored there.
    start: Option<(u16, u16)>,
    typed: Vec<crate::Event>,
    /// The title the application set, which a resume shows again.
    title: Option<String>,
    /// The progress the application showed, which a resume shows again.
    progress: Progress,
    /// Whether requests tmux would swallow need its passthrough wrapper.
    tmux: bool,
    probed: bool,
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
    /// The terminal is asked once, here, what it supports and where its cursor
    /// is; see `capabilities`.
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
            capabilities: Capabilities::default(),
            start: None,
            typed: Vec::new(),
            title: None,
            progress: Progress::Clear,
            tmux: std::env::var_os("TMUX").is_some(),
            probed: false,
            raw: false,
            entered: false,
        };
        if terminal::is_raw_mode_enabled()? {
            return Err(io::Error::other("terminal is already in raw mode"));
        }
        install_panic_hook();
        if options.signals {
            watch_signals();
        }
        let signals = if options.signals { RESCUE } else { HANDS_OFF };
        SIGNALS.store(signals, Ordering::Release);
        session.resume()?;
        Ok(session)
    }

    /// What the terminal reported at startup.
    pub fn capabilities(&self) -> &Capabilities {
        &self.capabilities
    }

    /// Keys typed while the terminal was being asked about itself: at startup,
    /// or when `switch` first enters an inline session. Handle them before
    /// reading input, so typing ahead of a slow terminal is not lost.
    pub fn typed_ahead(&mut self) -> Vec<crate::Event> {
        std::mem::take(&mut self.typed)
    }

    /// Restore terminal modes temporarily while retaining exclusive ownership.
    /// Resume before drawing again. This permits an application to run a child
    /// UI. An inline session first parks the cursor beneath its last frame.
    pub fn suspend(&mut self) -> io::Result<()> {
        let mut result = Ok(());
        // Whoever holds the modes undoes them; a panic hook may have already.
        // Keep rescue from observing a half-restored session.
        let mut active = active();
        let taken = active.take();
        if let Some(state) = taken.filter(|state| state.options.is_some()) {
            if state
                .options
                .is_some_and(|options| options.screen == ScreenMode::Inline)
            {
                if let Some(inline) = &mut self.inline {
                    result = inline.finish(&mut self.output);
                }
            }
            // The live frame parks the cursor, not the recorded one.
            let state = Active {
                park: Park::default(),
                ..state
            };
            result = result.and(state.leave(&mut self.output));
        }
        self.entered = false;
        if self.raw {
            let raw = terminal::disable_raw_mode();
            self.raw = raw.is_err();
            result = result.and(raw);
        }
        drop(active);
        self.invalidate();
        result
    }

    /// Reacquire modes after suspension and force a complete repaint.
    pub fn resume(&mut self) -> io::Result<()> {
        if self.raw && self.entered {
            return Ok(());
        }
        if !self.raw {
            // Publish raw-mode ownership before the probe can block. Holding
            // the lock closes the gap between acquiring it and registering it.
            let mut active = active();
            terminal::enable_raw_mode()?;
            self.raw = true;
            *active = Some(Active {
                options: None,
                park: Park::default(),
                surface: Surface::default(),
            });
        }
        // Raw escape sequences need virtual terminal processing on Windows.
        #[cfg(windows)]
        let _ = crossterm::ansi_support::supports_ansi();
        if !self.probed {
            self.probed = true;
            let probe = query::probe(&mut self.output)?;
            self.capabilities = probe.capabilities;
            self.start = probe.cursor;
            self.typed = probe.typed;
            // A request for key reports the terminal never answered is dropped.
            self.options.keyboard &= self.capabilities.keyboard;
        }
        self.entered = true;
        self.renderer.invalidate();
        match &mut self.inline {
            // Whatever ran during the suspension may have written anywhere.
            Some(inline) => inline.invalidate(),
            None if self.options.screen == ScreenMode::Inline => {
                let cursor = self.start.take();
                self.anchor(cursor)?;
            }
            None => {}
        }
        let state = self.restoration();
        let mut active = active();
        *active = Some(state);
        drop(active);
        self.options.enter(&mut self.output)?;
        // Whatever ran during a suspension had the user's title back.
        if let Some(title) = self.title.take() {
            self.title(&title)?;
        }
        if self.progress != Progress::Clear {
            self.progress(self.progress)?;
        }
        Ok(())
    }

    /// Keep what a crash would need to restore the terminal up to date.
    fn record(&self) {
        *active() = Some(self.restoration());
    }

    /// Cleanup for the entered screen and input modes, including cursor parking.
    fn restoration(&self) -> Active {
        let park = match (&self.inline, self.options.screen) {
            (Some(inline), ScreenMode::Inline) => inline.parking(),
            (_, ScreenMode::Alternate) => Park::default(),
            _ => Park {
                row: None,
                newline: true,
            },
        };
        Active {
            options: Some(self.options),
            park,
            surface: Surface {
                title: self.title.is_some(),
                progress: self.progress != Progress::Clear,
                tmux: self.tmux,
            },
        }
    }

    /// Map an event from the screen onto the frame. An inline frame starts
    /// wherever the session began and moves as it scrolls, so a mouse report's
    /// row is not the frame's row. Pass every event through this before
    /// dispatching it; one that falls outside an inline frame is `None`.
    pub fn to_frame(&self, event: crate::Event) -> Option<crate::Event> {
        match (event, &self.inline, self.options.screen) {
            (crate::Event::Mouse(mouse), Some(inline), ScreenMode::Inline) => {
                let y = inline.frame_row(mouse.y)?;
                Some(crate::Event::Mouse(crate::Mouse { y, ..mouse }))
            }
            (event, ..) => Some(event),
        }
    }

    /// Stop the process as Ctrl-Z does in a shell, and come back when it is
    /// continued. Raw mode turns the key into an ordinary event, so an
    /// application that wants job control calls this when it sees one.
    #[cfg(unix)]
    pub fn stop(&mut self) -> io::Result<()> {
        self.suspend()?;
        signal_hook::low_level::raise(signal_hook::consts::SIGTSTP)?;
        self.resume()
    }

    /// Turn the input modes on again. Windows consoles drop them while the
    /// window is unfocused, so call this on `Event::WindowFocus(true)` there.
    pub fn reassert(&mut self) -> io::Result<()> {
        if self.entered {
            self.options.reassert(&mut self.output)?;
        }
        Ok(())
    }

    /// Ask the terminal to put text on the system clipboard. It works over
    /// SSH and through tmux, and does nothing where the terminal declines.
    /// Nothing reads the clipboard back: pasted text arrives as
    /// `Event::Paste` while `Options::paste` is on.
    pub fn copy(&mut self, text: &str) -> io::Result<()> {
        self.output.write_all(&render::clipboard(text, self.tmux))?;
        self.output.flush()
    }

    /// Set the window and tab title. The user's own title is saved first and
    /// put back when the session ends or is suspended, where the terminal
    /// keeps a title stack. A title holding control characters, or longer
    /// than 2048 bytes, is refused with `InvalidInput`.
    pub fn title(&mut self, title: &str) -> io::Result<()> {
        let bytes = render::title(title).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "a title must be at most 2048 bytes without control characters",
            )
        })?;
        self.entered()?;
        let push = self.title.is_none();
        self.title = Some(title.to_owned());
        // Recorded before the push, so a crash in between still pops it.
        self.record();
        if push {
            self.output.write_all(PUSH_TITLE)?;
        }
        self.output.write_all(&bytes)?;
        self.output.flush()
    }

    /// Ring the terminal bell, for example when a long task finishes.
    pub fn bell(&mut self) -> io::Result<()> {
        self.output.write_all(render::BELL)?;
        self.output.flush()
    }

    /// Show progress in the tab or taskbar, where the terminal supports
    /// OSC 9;4. The indicator is cleared when the session ends or is
    /// suspended; `Progress::Clear` removes it sooner.
    pub fn progress(&mut self, progress: Progress) -> io::Result<()> {
        self.entered()?;
        self.progress = progress;
        self.record();
        self.output
            .write_all(&render::progress(progress, self.tmux))?;
        self.output.flush()
    }

    /// Output that the end of the session must undo needs an entered session.
    fn entered(&self) -> io::Result<()> {
        if self.raw && self.entered {
            Ok(())
        } else {
            Err(io::Error::other("terminal is suspended"))
        }
    }

    /// Start the inline region on a fresh line at the cursor. A terminal that
    /// does not report its cursor gets a cleared screen instead.
    fn anchor(&mut self, cursor: Option<(u16, u16)>) -> io::Result<()> {
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
    /// session drew while the alternate screen is up. Start inline when the
    /// session will be inline at all: anchoring later has to ask the terminal
    /// for its cursor and wait for the reply. Keys typed during that wait go
    /// to `typed_ahead`.
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
        // The main and alternate screens keep separate stacks of keyboard
        // modes, so ours is popped from the screen being left and pushed on the
        // one being entered. Otherwise the pop at exit misses, and the shell
        // inherits key reports it cannot read.
        let crossing = old == ScreenMode::Alternate || screen == ScreenMode::Alternate;
        if crossing && self.options.keyboard {
            self.output.write_all(b"\x1b[<1u")?;
        }
        if old == ScreenMode::Alternate {
            self.output.write_all(b"\x1b[?1049l")?;
        }
        if screen == ScreenMode::Alternate {
            self.output.write_all(b"\x1b[?1049h")?;
        }
        if crossing && self.options.keyboard {
            self.output.write_all(b"\x1b[>1u")?;
        }
        self.output.flush()?;
        match (&mut self.inline, screen, old) {
            (None, ScreenMode::Inline, _) => {
                let reply = query::cursor(&mut self.output)?;
                self.typed.extend(reply.typed);
                self.anchor(reply.cursor)?;
            }
            // Full-screen drawing on the main screen covered the frame.
            (Some(inline), ScreenMode::Inline, ScreenMode::Main) => inline.invalidate(),
            // Leaving the alternate screen restores the main screen's rows
            // but not the cursor the full-screen view hid or reshaped.
            (Some(inline), ScreenMode::Inline, ScreenMode::Alternate) => inline.forget_cursor(),
            _ => {}
        }
        self.record();
        Ok(())
    }

    /// Current terminal dimensions; read again after resize events.
    pub fn size(&self) -> io::Result<(u16, u16)> {
        terminal::size()
    }

    /// Paint a frame. A full-screen frame matches the terminal's dimensions.
    /// An inline frame matches its width and may be of any height.
    pub fn draw(&mut self, frame: &Buffer) -> io::Result<()> {
        self.entered()?;
        match (&mut self.inline, self.options.screen) {
            (Some(inline), ScreenMode::Inline) => {
                if inline.draw(&mut self.output, frame, terminal::size()?.1)? {
                    // The frame may have grown or scrolled; a crash parks beneath it.
                    self.record();
                }
                Ok(())
            }
            _ => self.renderer.draw(&mut self.output, frame),
        }
    }

    /// Inline only: release the first `rows` rows of the last frame to the
    /// terminal's history. See `Inline::commit`.
    pub fn commit(&mut self, rows: u16) {
        if let Some(inline) = &mut self.inline {
            inline.commit(rows);
            if self.entered {
                // Parking moves with the committed rows.
                self.record();
            }
        }
    }

    /// Repaint on the next draw after another owner wrote to stdout.
    pub fn invalidate(&mut self) {
        self.renderer.invalidate();
        if let Some(inline) = &mut self.inline {
            inline.invalidate();
        }
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        let _ = self.suspend();
        SIGNALS.store(DEFAULT, Ordering::Release);
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
            Some(Event::key(key, mods))
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
        event::Event::FocusGained => Some(Event::WindowFocus(true)),
        event::Event::FocusLost => Some(Event::WindowFocus(false)),
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
/// Input received during the startup probe is dispatched before new input.
pub fn run(
    tree: &mut crate::Tree,
    mut update: impl FnMut(&mut crate::Tree, &crate::Event, &crate::Dispatch) -> bool,
) -> io::Result<()> {
    let mut terminal = Terminal::new()?;
    let mut typed = terminal.typed_ahead().into_iter();
    loop {
        let (w, h) = terminal.size()?;
        terminal.draw(tree.frame(w, h).map_err(io::Error::other)?)?;
        let event = match typed.next() {
            Some(event) => Some(event),
            None => read()?,
        };
        let Some(event) = event else {
            continue;
        };
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

    fn leave(surface: Surface) -> String {
        let options = Options {
            screen: ScreenMode::Inline,
            ..Options::default()
        };
        let active = Active {
            options: Some(options),
            park: Park {
                row: Some(3),
                newline: true,
            },
            surface,
        };
        let mut output = Vec::new();
        active.leave(&mut output).unwrap();
        String::from_utf8(output).unwrap()
    }

    #[test]
    fn restoring_after_a_crash_takes_back_the_title_and_progress() {
        let output = leave(Surface {
            title: true,
            progress: true,
            tmux: false,
        });
        let park = output.find("\r\n").unwrap();
        let pop = output.find("\x1b[23;0t").unwrap();
        let clear = output.find("\x1b]9;4;0;0\x1b\\").unwrap();
        assert!(park < clear && park < pop, "{output:?}");
    }

    #[test]
    fn a_session_that_set_no_title_or_progress_leaves_them_alone() {
        let output = leave(Surface::default());
        assert!(
            !output.contains("23;0t") && !output.contains("]9;"),
            "{output:?}"
        );
    }
}
