//! What the terminal shows, and the queue that lets a thread of its own write
//! it while the session's owner keeps handling input.
//!
//! Frames are rendered where they are written: the writer takes the newest
//! queued frame, renders it against the last frame it rendered, and writes
//! the bytes with the lock released. A frame that is replaced before the
//! writer reaches it is never rendered, which is safe because the next frame
//! is diffed against what was actually sent. A commit is queued between the
//! frames it separates, so the rows it releases are drawn first.
use crate::render::Park;
use crate::{Buffer, Inline, Renderer, ScreenMode};
use std::collections::VecDeque;
use std::io::{self, Write};
use std::sync::{Condvar, Mutex, MutexGuard, PoisonError};

/// The renderers of a session and the screen they draw on.
pub(crate) struct Screen {
    pub renderer: Renderer,
    pub inline: Option<Inline>,
    pub mode: ScreenMode,
}

impl Screen {
    /// The bytes that show `frame` on a screen `height` rows tall.
    fn render(&mut self, frame: &Buffer, height: u16) -> &[u8] {
        match (&mut self.inline, self.mode) {
            (Some(inline), ScreenMode::Inline) => inline.render(frame, height),
            _ => self.renderer.render(frame),
        }
    }

    /// Whether rendering `frame` would send nothing without comparing cells.
    fn current(&self, frame: &Buffer, height: u16) -> bool {
        match (&self.inline, self.mode) {
            (Some(inline), ScreenMode::Inline) => inline.current(frame, height),
            _ => self.renderer.current(frame),
        }
    }

    /// Forget what the terminal shows, after a failed write or another writer.
    pub fn invalidate(&mut self) {
        self.renderer.invalidate();
        if let Some(inline) = &mut self.inline {
            inline.invalidate();
        }
    }

    /// Release the first `rows` rows of the last inline frame.
    pub fn commit(&mut self, rows: u16) {
        if let Some(inline) = &mut self.inline {
            inline.commit(rows);
        }
    }

    /// Where a crash leaves the cursor: beneath an inline frame, on a fresh
    /// line after full-screen drawing on the main screen, and anywhere on
    /// the alternate screen, which the terminal restores.
    pub fn park(&self) -> Park {
        match (&self.inline, self.mode) {
            (Some(inline), ScreenMode::Inline) => inline.parking(),
            (_, ScreenMode::Alternate) => Park::default(),
            _ => Park {
                row: None,
                newline: true,
            },
        }
    }
}

/// Output waiting for the writer, in order.
enum Op {
    /// A frame and the screen height it was drawn for.
    Frame(Buffer, u16),
    Commit(u16),
}

struct State {
    screen: Screen,
    queue: VecDeque<Op>,
    /// A frame's bytes are on their way to the terminal.
    writing: bool,
    /// The first failed write since the owner last asked.
    error: Option<io::Error>,
    /// Storage of a written frame, reused for the next one queued.
    spare: Option<Buffer>,
    /// The session is ending: the writer stops once the queue is empty.
    closed: bool,
    /// The writer has returned, or unwound from a panic.
    stopped: bool,
}

/// The screen shared by the session's owner and its writer.
pub(crate) struct Output {
    state: Mutex<State>,
    changed: Condvar,
}

impl Output {
    pub fn new(screen: Screen) -> Self {
        Self {
            state: Mutex::new(State {
                screen,
                queue: VecDeque::new(),
                writing: false,
                error: None,
                spare: None,
                closed: false,
                stopped: false,
            }),
            changed: Condvar::new(),
        }
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Use the screen directly. Hold it only briefly: the writer renders
    /// under the same lock.
    pub fn with<T>(&self, use_screen: impl FnOnce(&mut Screen) -> T) -> T {
        use_screen(&mut self.lock().screen)
    }

    /// Render and write `frame` on the calling thread, invalidating the
    /// screen if the write fails. Returns whether anything was written.
    pub fn draw(&self, sink: &mut impl Write, frame: &Buffer, height: u16) -> io::Result<bool> {
        let mut state = self.lock();
        let bytes = state.screen.render(frame, height);
        if bytes.is_empty() {
            return Ok(false);
        }
        let result = sink.write_all(bytes).and_then(|()| sink.flush());
        result.inspect_err(|_| state.screen.invalidate())?;
        Ok(true)
    }

    /// Queue `frame` for the writer, replacing a queued frame it has not
    /// reached yet. Reports a write that failed since the last call.
    pub fn queue(&self, frame: &Buffer, height: u16) -> io::Result<()> {
        let mut state = self.lock();
        if let Some(error) = state.error.take() {
            return Err(error);
        }
        let state = &mut *state;
        match state.queue.back_mut() {
            Some(Op::Frame(queued, queued_height)) => {
                if !(same(queued, frame) && *queued_height == height) {
                    queued.clone_from(frame);
                    *queued_height = height;
                }
            }
            None if state.screen.current(frame, height) => return Ok(()),
            _ => {
                let buffer = match state.spare.take() {
                    Some(mut buffer) => {
                        buffer.clone_from(frame);
                        buffer
                    }
                    None => frame.clone(),
                };
                state.queue.push_back(Op::Frame(buffer, height));
                self.changed.notify_all();
            }
        }
        Ok(())
    }

    /// Queue a commit behind the frames already queued.
    pub fn commit(&self, rows: u16) {
        self.lock().queue.push_back(Op::Commit(rows));
        self.changed.notify_all();
    }

    /// Wait until everything queued has been written or discarded, and
    /// report a write that failed since the last call.
    pub fn flush(&self) -> io::Result<()> {
        let mut state = self.lock();
        while (state.writing || !state.queue.is_empty()) && !state.stopped {
            state = self
                .changed
                .wait(state)
                .unwrap_or_else(PoisonError::into_inner);
        }
        state.error.take().map_or(Ok(()), Err)
    }

    /// Stop the writer once the queue is empty.
    pub fn close(&self) {
        self.lock().closed = true;
        self.changed.notify_all();
    }

    /// Write queued output to `sink` until `close`. Before each write,
    /// `publish` receives where a crash should park the cursor, and returns
    /// whether the session still owns the terminal; when it does not, the
    /// bytes are dropped rather than drawn over a restored shell.
    pub fn write(&self, sink: &mut impl Write, publish: impl Fn(Park) -> bool) {
        /// Marks the writer stopped however it returns, so `flush` never
        /// waits for a writer that has gone.
        struct Stopped<'a>(&'a Output);
        impl Drop for Stopped<'_> {
            fn drop(&mut self) {
                self.0.lock().stopped = true;
                self.0.changed.notify_all();
            }
        }
        let _stopped = Stopped(self);
        let mut bytes = Vec::new();
        let mut state = self.lock();
        loop {
            let Some(op) = state.queue.pop_front() else {
                if state.closed {
                    return;
                }
                state = self
                    .changed
                    .wait(state)
                    .unwrap_or_else(PoisonError::into_inner);
                continue;
            };
            bytes.clear();
            match op {
                Op::Frame(frame, height) => {
                    bytes.extend_from_slice(state.screen.render(&frame, height));
                    state.spare = Some(frame);
                }
                Op::Commit(rows) => state.screen.commit(rows),
            }
            let park = state.screen.park();
            state.writing = true;
            drop(state);
            let live = publish(park);
            let result = if bytes.is_empty() || !live {
                Ok(())
            } else {
                sink.write_all(&bytes).and_then(|()| sink.flush())
            };
            state = self.lock();
            state.writing = false;
            if let Err(error) = result {
                state.screen.invalidate();
                state.error.get_or_insert(error);
            }
            self.changed.notify_all();
        }
    }
}

/// Whether two frames are known to be the same without comparing cells.
fn same(a: &Buffer, b: &Buffer) -> bool {
    a.version != 0 && a.version == b.version
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Depth, Rect, Style};
    use std::sync::mpsc;

    /// A terminal that takes the first frame, then stalls until released,
    /// as a slow connection does.
    struct Stalled {
        writes: mpsc::Sender<Vec<u8>>,
        release: Option<mpsc::Receiver<()>>,
    }
    impl Write for Stalled {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.writes.send(bytes.to_vec()).unwrap();
            if let Some(release) = self.release.take() {
                release.recv().unwrap();
            }
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    fn frame(rows: &[&str]) -> Buffer {
        let mut frame = Buffer::new(12, rows.len() as u16);
        for (y, row) in rows.iter().enumerate() {
            frame.write(Rect::new(0, y as u16, 12, 1), row, Style::default());
        }
        frame
    }

    /// Queue the first frame, stall the writer on it, then queue `behind`,
    /// and return every write the terminal received.
    fn fall_behind(screen: Screen, first: &Buffer, behind: impl FnOnce(&Output)) -> Vec<Vec<u8>> {
        let output = Output::new(screen);
        let (writes, written) = mpsc::channel();
        let (release, released) = mpsc::channel();
        let mut sink = Stalled {
            writes,
            release: Some(released),
        };
        let mut writes = Vec::new();
        std::thread::scope(|scope| {
            scope.spawn(|| output.write(&mut sink, |_| true));
            output.queue(first, 4).unwrap();
            // The writer is inside the first write when it arrives here.
            writes.push(written.recv().unwrap());
            behind(&output);
            release.send(()).unwrap();
            output.flush().unwrap();
            output.close();
        });
        drop(sink);
        writes.extend(written.iter());
        writes
    }

    #[test]
    fn a_writer_that_falls_behind_draws_only_the_newest_frame_as_a_diff() {
        let screen = Screen {
            renderer: Renderer::with_depth(Depth::Rgb),
            inline: None,
            mode: ScreenMode::Alternate,
        };
        let frames = [["one"], ["two"], ["three"], ["four"]].map(|rows| frame(&rows));
        let writes = fall_behind(screen, &frames[0], |output| {
            for frame in &frames[1..] {
                output.queue(frame, 4).unwrap();
            }
        });
        // The terminal ends up exactly where drawing every frame would
        // leave it: the skipped frames change nothing that is sent.
        let mut expected = Renderer::with_depth(Depth::Rgb);
        assert_eq!(
            writes,
            [
                expected.render(&frames[0]).to_vec(),
                expected.render(&frames[3]).to_vec()
            ]
        );
    }

    #[test]
    fn a_commit_keeps_its_place_between_the_frames_it_separates() {
        let screen = || Screen {
            renderer: Renderer::with_depth(Depth::Rgb),
            inline: Some(Inline::new(0, Depth::Rgb)),
            mode: ScreenMode::Inline,
        };
        let first = frame(&["a"]);
        let finished = frame(&["a", "b", "c"]);
        let (skipped, last) = (frame(&["b", "c", "d"]), frame(&["b", "c", "d", "e"]));
        let writes = fall_behind(screen(), &first, |output| {
            output.queue(&finished, 4).unwrap();
            output.commit(1);
            output.queue(&skipped, 4).unwrap();
            output.queue(&last, 4).unwrap();
        });
        let mut expected = screen();
        let mut replay = vec![expected.render(&first, 4).to_vec()];
        replay.push(expected.render(&finished, 4).to_vec());
        expected.commit(1);
        replay.push(expected.render(&last, 4).to_vec());
        assert_eq!(writes, replay);
    }
}
