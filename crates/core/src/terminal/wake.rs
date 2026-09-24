//! Waking a loop that waits for terminal input, so work from other threads
//! is drawn without polling on a timer.
//!
//! crossterm waits on the terminal alone and cannot be handed another source
//! to watch. On Unix, once a `Waker` exists, `poll` and `read` wait in
//! `poll(2)` on the terminal and a socket pair instead, and ask crossterm for
//! an event only once something is ready. Wakes and window-size signals both
//! write to the pair; a flag tells a wake from a resize. Windows has no such
//! wait, so there the wait is cut into short slices that check the flag.
use crossterm::event;
use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

/// Ends a wait in `terminal::poll` or `terminal::read` from any thread.
/// `poll` then returns true, and `read` returns `Ok(None)`, so a loop that
/// draws after every iteration takes its work and draws. Wakes that arrive
/// before the loop waits again are merged into one. Only those two waits
/// see a wake: a loop on crossterm's own `read` or on `EventStream` does
/// not, and `poll` keeps reporting the wake until `read` takes it.
#[derive(Clone, Copy, Debug)]
pub struct Waker {
    channel: &'static Channel,
}

impl Waker {
    /// Wake the thread waiting for input, or the next one to wait.
    pub fn wake(&self) {
        self.channel.requested.store(true, Ordering::Release);
        #[cfg(unix)]
        {
            use std::io::Write;
            // A full socket already holds a pending wake.
            let _ = (&self.channel.write).write(&[1]);
        }
    }
}

#[derive(Debug)]
struct Channel {
    requested: AtomicBool,
    #[cfg(unix)]
    read: std::os::unix::net::UnixStream,
    #[cfg(unix)]
    write: std::os::unix::net::UnixStream,
}

static CHANNEL: OnceLock<Channel> = OnceLock::new();

/// A handle that ends the current or next wait for terminal input. There is
/// one channel per process, made on first use; every waker shares it.
pub fn waker() -> io::Result<Waker> {
    if let Some(channel) = CHANNEL.get() {
        return Ok(Waker { channel });
    }
    #[cfg(unix)]
    let channel = {
        let (read, write) = std::os::unix::net::UnixStream::pair()?;
        read.set_nonblocking(true)?;
        write.set_nonblocking(true)?;
        Channel {
            requested: AtomicBool::new(false),
            read,
            write,
        }
    };
    #[cfg(not(unix))]
    let channel = Channel {
        requested: AtomicBool::new(false),
    };
    if CHANNEL.set(channel).is_ok() {
        // Signal actions run in registration order, and crossterm registers
        // its own resize action when it first waits for input. Ours must
        // come after it, or a wait can wake before crossterm has noted the
        // resize and go back to sleep with it unread.
        let _ = event::poll(Duration::ZERO);
        watch_resizes()?;
    }
    let channel = CHANNEL
        .get()
        .ok_or_else(|| io::Error::other("no wake channel"))?;
    Ok(Waker { channel })
}

/// crossterm learns of a resize from a signal; without this, a wait on the
/// socket and the terminal would not see it.
#[cfg(unix)]
fn watch_resizes() -> io::Result<()> {
    if let Some(channel) = CHANNEL.get() {
        let write = channel.write.try_clone()?;
        signal_hook::low_level::pipe::register(signal_hook::consts::SIGWINCH, write)?;
    }
    Ok(())
}
#[cfg(not(unix))]
fn watch_resizes() -> io::Result<()> {
    Ok(())
}

/// What ended a wait.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Ready {
    Input,
    Wake,
    Timeout,
}

/// Wait up to `timeout`, or indefinitely, for terminal input or a wake.
/// `take` consumes the wake it reports; otherwise the next wait sees it too.
/// Without a waker this is crossterm's own wait.
pub(crate) fn wait(timeout: Option<Duration>, take: bool) -> io::Result<Ready> {
    let Some(channel) = CHANNEL.get() else {
        let ready = match timeout {
            Some(timeout) => event::poll(timeout)?,
            None => true,
        };
        return Ok(if ready { Ready::Input } else { Ready::Timeout });
    };
    let deadline = timeout.and_then(|timeout| Instant::now().checked_add(timeout));
    loop {
        #[cfg(unix)]
        drain(&channel.read);
        let woken = if take {
            channel.requested.swap(false, Ordering::AcqRel)
        } else {
            channel.requested.load(Ordering::Acquire)
        };
        if woken {
            return Ok(Ready::Wake);
        }
        // crossterm may hold events it has already read.
        if event::poll(Duration::ZERO)? {
            return Ok(Ready::Input);
        }
        let left = deadline.map(|deadline| deadline.saturating_duration_since(Instant::now()));
        if left.is_some_and(|left| left.is_zero()) {
            return Ok(Ready::Timeout);
        }
        #[cfg(unix)]
        block(channel, left)?;
        #[cfg(not(unix))]
        {
            // Short enough that a wake is prompt, long enough to stay idle.
            const SLICE: Duration = Duration::from_millis(20);
            if event::poll(left.map_or(SLICE, |left| left.min(SLICE)))? {
                return Ok(Ready::Input);
            }
        }
    }
}

/// Discard the bytes wakes and resizes wrote; the flag says which it was.
#[cfg(unix)]
fn drain(mut read: &std::os::unix::net::UnixStream) {
    use std::io::Read;
    let mut bytes = [0; 64];
    while matches!(read.read(&mut bytes), Ok(n) if n > 0) {}
}

/// Sleep until the terminal or the socket has something to read.
#[cfg(unix)]
fn block(channel: &Channel, left: Option<Duration>) -> io::Result<()> {
    use rustix::event::{poll, PollFd, PollFlags, Timespec};
    let timeout = left.map(|left| Timespec {
        tv_sec: left.as_secs().try_into().unwrap_or(i64::MAX),
        tv_nsec: left.subsec_nanos().into(),
    });
    let stdin = rustix::stdio::stdin();
    let mut fds = [
        PollFd::new(&stdin, PollFlags::IN),
        PollFd::new(&channel.read, PollFlags::IN),
    ];
    match poll(&mut fds, timeout.as_ref()) {
        Ok(_) | Err(rustix::io::Errno::INTR) => Ok(()),
        Err(error) => Err(error.into()),
    }
}
