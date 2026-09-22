//! Run a remote application without acquiring the server terminal.
use crate::{server::Factory, Error, Peer};
use russh::{server::Msg, ChannelWriteHalf};
use std::{
    mem,
    panic::{catch_unwind, AssertUnwindSafe},
    sync::{Arc, Mutex, PoisonError},
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
    let mut writing: Option<JoinHandle<Result<(), Error>>> = None;
    // The same modes a local terminal session enables, carried over the channel.
    let modes = Options::default();
    let result = catch_unwind(AssertUnwindSafe(|| -> Result<(), Error> {
        let mut app = factory(&peer)?;
        let (mut width, mut height) = (peer.width, peer.height);
        // The peer's terminal type is all that is known of its colors.
        let term = peer.term.clone();
        let depth = Depth::from_env(|name| (name == "TERM").then(|| term.clone()));
        let mut renderer = Renderer::with_depth(depth);
        let mut decoder = Decoder::default();
        // The modes go out with the first frame.
        let mut first = Vec::new();
        modes.enter(&mut first)?;
        let mut dirty = true;
        loop {
            if dirty && writing.is_none() {
                // The renderer assumes its bytes arrive; a failed write ends
                // the session, so there is no later frame to invalidate.
                let bytes = renderer.render(app.frame(width, height)?);
                dirty = false;
                let bytes = if first.is_empty() {
                    chunks(bytes)
                } else {
                    first.extend_from_slice(bytes);
                    chunks(&mem::take(&mut first))
                };
                if !bytes.is_empty() {
                    writing = Some(runtime.spawn(write(output.clone(), bytes)));
                }
            }
            let escape = decoder.escape_pending();
            let wake = runtime.block_on(async {
                let written = async {
                    match &mut writing {
                        Some(writing) => finish(writing).await,
                        None => std::future::pending().await,
                    }
                };
                tokio::select! {
                    result = written => Wake::Written(result),
                    () = input.ready.notified() => Wake::Input,
                    () = tokio::time::sleep(Duration::from_millis(40)), if escape => Wake::Escape,
                }
            });
            let mut events = Vec::new();
            let mut closed = false;
            match wake {
                Wake::Written(result) => {
                    writing = None;
                    result?;
                }
                Wake::Escape => events = decoder.flush_escape(),
                Wake::Input => {
                    let pending = input.take();
                    if let Some((w, h)) = pending.resize {
                        (width, height) = (w, h);
                        events.push(Event::Resize(w, h));
                    }
                    events.extend(decoder.push(&pending.data));
                    closed = pending.closed;
                }
            }
            for event in events {
                dirty = true;
                if !app.event(event)? {
                    return Ok(());
                }
            }
            if closed {
                break;
            }
        }
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
