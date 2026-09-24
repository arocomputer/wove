//! Run a remote application without acquiring the server terminal.
use crate::{server::Factory, App, Error, Peer};
use russh::{server::Msg, ChannelWriteHalf};
use std::{
    mem,
    ops::ControlFlow,
    panic::{catch_unwind, AssertUnwindSafe},
    sync::{Arc, Mutex, PoisonError, Weak},
    time::Duration,
};
use tokio::{sync::Notify, task::JoinHandle};
use wove::{input::Decoder, Depth, Event, Options, Renderer};

/// The most input bytes held for an application that has not read them yet.
const INPUT_LIMIT: usize = 1 << 20;
/// How long an output write may wait for the peer to grant window space.
const STALL: Duration = Duration::from_secs(5);
/// The output unit a stall deadline covers.
const CHUNK: usize = 32 * 1024;

/// Input the connection has received and the application has not yet read.
/// The network side merges into it without waiting, so a busy application never
/// blocks the SSH session; the application takes everything at once.
#[derive(Default)]
pub(crate) struct Inbox {
    pending: Mutex<Pending>,
    ready: Notify,
}

#[derive(Default)]
struct Pending {
    data: Vec<u8>,
    resize: Option<(u16, u16)>,
    /// A `Waker` asked for a new frame.
    woken: bool,
    closed: bool,
}

impl Inbox {
    /// Append terminal input bytes, failing once unread input exceeds its bound.
    pub(crate) fn data(&self, bytes: &[u8]) -> Result<(), Error> {
        let mut pending = self.lock();
        if pending.data.len() + bytes.len() > INPUT_LIMIT {
            return Err("unread SSH input exceeds 1 MiB".into());
        }
        pending.data.extend_from_slice(bytes);
        drop(pending);
        self.ready.notify_one();
        Ok(())
    }
    /// Record new dimensions; only the latest unread size is delivered.
    pub(crate) fn resize(&self, width: u16, height: u16) {
        self.lock().resize = Some((width, height));
        self.ready.notify_one();
    }
    /// Ask the application thread for a new frame.
    fn wake(&self) {
        self.lock().woken = true;
        self.ready.notify_one();
    }
    /// End the application after it reads the remaining input.
    pub(crate) fn close(&self) {
        self.lock().closed = true;
        self.ready.notify_one();
    }
    fn take(&self) -> Pending {
        let mut pending = self.lock();
        let closed = pending.closed;
        Pending {
            closed,
            ..mem::take(&mut *pending)
        }
    }
    fn lock(&self) -> std::sync::MutexGuard<'_, Pending> {
        self.pending.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Asks a remote application for a new frame from any thread, so work that
/// finishes while the application is idle, such as a background task or a
/// timer, reaches the peer without waiting for input. The application thread
/// calls `App::frame`, where the application takes that work; wakes that
/// arrive before it does are merged into one. Waking a session that has ended
/// does nothing.
#[derive(Clone)]
pub struct Waker(Weak<Inbox>);

impl Waker {
    pub(crate) fn new(inbox: &Arc<Inbox>) -> Self {
        Self(Arc::downgrade(inbox))
    }

    pub fn wake(&self) {
        if let Some(inbox) = self.0.upgrade() {
            inbox.wake();
        }
    }
}

impl std::fmt::Debug for Waker {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("Waker").finish_non_exhaustive()
    }
}

/// Owned pieces of `bytes`, each covered by one stall deadline. This is the
/// only copy: the channel takes each piece without copying it again.
fn chunks(bytes: &[u8]) -> Vec<Vec<u8>> {
    bytes.chunks(CHUNK).map(<[u8]>::to_vec).collect()
}

/// Send chunks as the peer's channel window allows. Each chunk that waits
/// longer than `STALL` for window space fails the write, so a peer that stops
/// reading cannot hold the application indefinitely.
async fn write(output: Arc<ChannelWriteHalf<Msg>>, chunks: Vec<Vec<u8>>) -> Result<(), Error> {
    for chunk in chunks {
        tokio::time::timeout(STALL, output.data_bytes(chunk))
            .await
            .map_err(|_| "SSH output stalled")?
            .map_err(|_| "SSH channel closed")?;
    }
    Ok(())
}

/// Wait for a started write and report its failure.
async fn finish(writing: &mut JoinHandle<Result<(), Error>>) -> Result<(), Error> {
    writing.await.map_err(|_| "SSH output task failed")?
}

/// What woke the application thread.
enum Wake {
    Input,
    Written(Result<(), Error>),
    Escape,
}

/// Keep non-Send application state off the network executor. Input is merged
/// into a bounded inbox and at most one frame is in flight: while the peer is
/// slow, events keep updating the application and only its latest state is
/// drawn once the previous write completes. Application panics unwind inside
/// this boundary so remote mode restoration still runs.
pub(crate) fn run(factory: Factory, peer: Peer, input: Arc<Inbox>, output: ChannelWriteHalf<Msg>) {
    let runtime = tokio::runtime::Handle::current();
    let output = Arc::new(output);
    // Declared outside the boundary so that a frame still being written when
    // the application panics is waited for before the modes are left.
    let mut writing: Option<JoinHandle<Result<(), Error>>> = None;
    // The same modes a local terminal session enables, carried over the channel.
    let modes = Options::default();
    let result = catch_unwind(AssertUnwindSafe(|| -> Result<(), Error> {
        let mut session = Session::start(&factory, &peer, &runtime, &output, &modes, &mut writing)?;
        while session.step(&input)?.is_continue() {}
        Ok(())
    }))
    .unwrap_or_else(|_| Err("SSH application panicked".into()));
    let _ = runtime.block_on(async {
        if let Some(writing) = &mut writing {
            finish(writing).await?;
        }
        let mut bytes = Vec::new();
        modes.leave(&mut bytes)?;
        write(output.clone(), chunks(&bytes)).await?;
        Ok::<(), Error>(())
    });
    let _ = runtime.block_on(async {
        tokio::time::timeout(STALL, async {
            let _ = output.exit_status(u32::from(result.is_err())).await;
            let _ = output.eof().await;
            let _ = output.close().await;
        })
        .await
    });
}

/// One application on its thread: what it needs to draw a frame and to turn
/// the peer's bytes into events.
struct Session<'a> {
    app: Box<dyn App>,
    width: u16,
    height: u16,
    renderer: Renderer,
    decoder: Decoder,
    /// The mode-entering bytes, sent ahead of the first frame.
    first: Vec<u8>,
    /// A frame is owed: something changed since the last one was sent.
    dirty: bool,
    writing: &'a mut Option<JoinHandle<Result<(), Error>>>,
    runtime: &'a tokio::runtime::Handle,
    output: &'a Arc<ChannelWriteHalf<Msg>>,
}

impl<'a> Session<'a> {
    fn start(
        factory: &Factory,
        peer: &Peer,
        runtime: &'a tokio::runtime::Handle,
        output: &'a Arc<ChannelWriteHalf<Msg>>,
        modes: &Options,
        writing: &'a mut Option<JoinHandle<Result<(), Error>>>,
    ) -> Result<Self, Error> {
        let app = factory(peer)?;
        // The peer's terminal type is all that is known of its colors.
        let term = peer.term.clone();
        let depth = Depth::from_env(|name| (name == "TERM").then(|| term.clone()));
        let mut first = Vec::new();
        modes.enter(&mut first)?;
        Ok(Self {
            app,
            width: peer.width,
            height: peer.height,
            renderer: Renderer::with_depth(depth),
            decoder: Decoder::default(),
            first,
            dirty: true,
            writing,
            runtime,
            output,
        })
    }

    /// Send a frame if one is owed, wait for the next thing to happen, and
    /// deliver it to the application. Breaks when the application ends or
    /// the peer has closed the channel.
    fn step(&mut self, input: &Inbox) -> Result<ControlFlow<()>, Error> {
        self.send_frame()?;
        let (events, closed) = match self.wait(input) {
            Wake::Written(result) => {
                *self.writing = None;
                result?;
                (Vec::new(), false)
            }
            Wake::Escape => (self.decoder.flush_escape(), false),
            Wake::Input => self.take_input(input),
        };
        for event in events {
            self.dirty = true;
            if !self.app.event(event)? {
                return Ok(ControlFlow::Break(()));
            }
        }
        Ok(if closed {
            ControlFlow::Break(())
        } else {
            ControlFlow::Continue(())
        })
    }

    /// Render and start writing a frame, unless one is still being written.
    /// The renderer assumes its bytes arrive; a failed write ends the
    /// session, so there is no later frame to invalidate.
    fn send_frame(&mut self) -> Result<(), Error> {
        if !self.dirty || self.writing.is_some() {
            return Ok(());
        }
        let bytes = self
            .renderer
            .render(self.app.frame(self.width, self.height)?);
        self.dirty = false;
        let bytes = if self.first.is_empty() {
            chunks(bytes)
        } else {
            self.first.extend_from_slice(bytes);
            chunks(&mem::take(&mut self.first))
        };
        if !bytes.is_empty() {
            *self.writing = Some(self.runtime.spawn(write(self.output.clone(), bytes)));
        }
        Ok(())
    }

    /// Block until the frame in flight is written, input arrives, or a lone
    /// Escape has waited long enough to be a key of its own.
    fn wait(&mut self, input: &Inbox) -> Wake {
        let escape = self.decoder.escape_pending();
        let writing = &mut *self.writing;
        self.runtime.block_on(async {
            let written = async {
                match writing {
                    Some(writing) => finish(writing).await,
                    None => std::future::pending().await,
                }
            };
            tokio::select! {
                result = written => Wake::Written(result),
                () = input.ready.notified() => Wake::Input,
                () = tokio::time::sleep(Duration::from_millis(40)), if escape => Wake::Escape,
            }
        })
    }

    /// Everything the peer sent since the last take, as events, and whether
    /// the channel has closed behind it.
    fn take_input(&mut self, input: &Inbox) -> (Vec<Event>, bool) {
        let pending = input.take();
        let mut events = Vec::new();
        if let Some((width, height)) = pending.resize {
            (self.width, self.height) = (width, height);
            // The peer's terminal may have moved rows even when the size is
            // back to what it was, so a resize always repaints.
            self.renderer.invalidate();
            events.push(Event::Resize(width, height));
        }
        events.extend(self.decoder.push(&pending.data));
        self.dirty |= pending.woken;
        (events, pending.closed)
    }
}
