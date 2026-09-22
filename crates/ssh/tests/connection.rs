//! Exercise the encrypted transport with independent client and server state.
use russh::{
    client,
    keys::{Algorithm, PrivateKeyWithHashAlg, PublicKeyOrCertificate},
    Channel, ChannelMsg,
};
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::{
    net::TcpListener,
    sync::{mpsc, oneshot},
    time::timeout,
};
use wove::{
    elements::{Input, Text},
    Buffer, Event, Tree,
};
use wove_ssh::{App, Error, PrivateKey, PublicKey, Server};

struct Client(PublicKey);
impl client::Handler for Client {
    type Error = russh::Error;
    async fn check_server_key(
        &mut self,
        key: &PublicKeyOrCertificate,
    ) -> Result<bool, Self::Error> {
        Ok(matches!(key, PublicKeyOrCertificate::PublicKey {key, ..} if key == &self.0))
    }
}

struct Editor {
    tree: Tree,
    seen: mpsc::UnboundedSender<Event>,
    alive: Arc<AtomicUsize>,
}
impl App for Editor {
    fn frame(&mut self, width: u16, height: u16) -> Result<&Buffer, Error> {
        Ok(self.tree.frame(width, height)?)
    }
    fn event(&mut self, event: Event) -> Result<bool, Error> {
        self.seen.send(event.clone())?;
        if event == ctrl('c') {
            return Ok(false);
        }
        self.tree.dispatch(event)?;
        Ok(true)
    }
}
impl Drop for Editor {
    fn drop(&mut self) {
        self.alive.fetch_sub(1, Ordering::SeqCst);
    }
}

fn ctrl(c: char) -> Event {
    let ctrl = wove::Modifiers {
        ctrl: true,
        ..Default::default()
    };
    Event::Key(wove::Key::Char(c), ctrl)
}

fn key() -> PrivateKey {
    PrivateKey::random(&mut rand::rng(), Algorithm::Ed25519).unwrap()
}

async fn message(channel: &mut Channel<client::Msg>) -> ChannelMsg {
    timeout(Duration::from_secs(5), channel.wait())
        .await
        .unwrap()
        .unwrap()
}

async fn output(channel: &mut Channel<client::Msg>, needle: &[u8]) -> Vec<u8> {
    timeout(Duration::from_secs(5), async {
        let mut bytes = Vec::new();
        loop {
            if let ChannelMsg::Data { data } = message(channel).await {
                bytes.extend_from_slice(&data);
            }
            if bytes.windows(needle.len()).any(|part| part == needle) {
                return bytes;
            }
        }
    })
    .await
    .unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn authentication_input_resize_isolation_and_shutdown() {
    timeout(Duration::from_secs(20), async {
        let host = key();
        let host_public = host.public_key().clone();
        let allowed = Arc::new(key());
        let public = allowed.public_key().clone();
        let alive = Arc::new(AtomicUsize::new(0));
        let count = alive.clone();
        let (seen, mut events) = mpsc::unbounded_channel();
        let server = Server::new(
            host,
            move |user, key| user == "guest" && key == &public,
            move |_| {
                let mut tree = Tree::new();
                let input = tree.add(tree.root(), Input::default())?;
                tree.focus(Some(input))?;
                count.fetch_add(1, Ordering::SeqCst);
                Ok(Editor {
                    tree,
                    seen: seen.clone(),
                    alive: count.clone(),
                })
            },
        );
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (stop, stopped) = oneshot::channel();
        let running = tokio::spawn(server.serve(listener, async {
            let _ = stopped.await;
        }));
        let mut first = client::connect(
            Arc::new(client::Config::default()),
            address,
            Client(host_public.clone()),
        )
        .await
        .unwrap();
        assert!(!first
            .authenticate_publickey("guest", PrivateKeyWithHashAlg::new(Arc::new(key()), None))
            .await
            .unwrap()
            .success());
        assert_eq!(alive.load(Ordering::SeqCst), 0);
        assert!(first
            .authenticate_publickey("guest", PrivateKeyWithHashAlg::new(allowed.clone(), None))
            .await
            .unwrap()
            .success());
        let mut channel = first.channel_open_session().await.unwrap();
        channel.exec(true, "uname").await.unwrap();
        assert!(matches!(message(&mut channel).await, ChannelMsg::Failure));
        channel
            .request_pty(true, "xterm-256color", 65535, 24, 0, 0, &[])
            .await
            .unwrap();
        assert!(matches!(message(&mut channel).await, ChannelMsg::Failure));
        channel
            .request_pty(true, "xterm-256color", 40, 8, 0, 0, &[])
            .await
            .unwrap();
        assert!(matches!(message(&mut channel).await, ChannelMsg::Success));
        channel.request_shell(true).await.unwrap();
        output(&mut channel, b"\x1b[?1049h").await;
        channel.data(&b"abc"[..]).await.unwrap();
        output(&mut channel, b"abc").await;
        for expected in ['a', 'b', 'c'] {
            assert_eq!(
                events.recv().await.unwrap(),
                wove::Key::Char(expected).into()
            );
        }
        channel.window_change(60, 12, 0, 0).await.unwrap();
        assert_eq!(events.recv().await.unwrap(), Event::Resize(60, 12));
        let mut second = client::connect(
            Arc::new(client::Config::default()),
            address,
            Client(host_public),
        )
        .await
        .unwrap();
        assert!(second
            .authenticate_publickey("guest", PrivateKeyWithHashAlg::new(allowed, None))
            .await
            .unwrap()
            .success());
        let mut other = second.channel_open_session().await.unwrap();
        other
            .request_pty(true, "xterm", 40, 8, 0, 0, &[])
            .await
            .unwrap();
        assert!(matches!(message(&mut other).await, ChannelMsg::Success));
        other.request_shell(true).await.unwrap();
        let initial = output(&mut other, b"\x1b[?1049h").await;
        assert!(!initial.windows(3).any(|part| part == b"abc"));
        other.data(&b"z"[..]).await.unwrap();
        output(&mut other, b"z").await;
        assert_eq!(events.recv().await.unwrap(), wove::Key::Char('z').into());
        assert_eq!(alive.load(Ordering::SeqCst), 2);
        channel.data(&b"\x03"[..]).await.unwrap();
        output(&mut channel, b"\x1b[?1049l").await;
        stop.send(()).unwrap();
        running.await.unwrap().unwrap();
        while alive.load(Ordering::SeqCst) != 0 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancelling_server_releases_live_applications() {
    timeout(Duration::from_secs(10), async {
        let host = key();
        let host_public = host.public_key().clone();
        let allowed = Arc::new(key());
        let public = allowed.public_key().clone();
        let alive = Arc::new(AtomicUsize::new(0));
        let count = alive.clone();
        let (seen, _events) = mpsc::unbounded_channel();
        let server = Server::new(
            host,
            move |_, key| key == &public,
            move |_| {
                count.fetch_add(1, Ordering::SeqCst);
                Ok(Editor {
                    tree: Tree::new(),
                    seen: seen.clone(),
                    alive: count.clone(),
                })
            },
        );
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let running = tokio::spawn(server.serve(listener, std::future::pending()));
        let mut client = client::connect(
            Arc::new(client::Config::default()),
            address,
            Client(host_public),
        )
        .await
        .unwrap();
        assert!(client
            .authenticate_publickey("guest", PrivateKeyWithHashAlg::new(allowed, None))
            .await
            .unwrap()
            .success());
        let mut channel = client.channel_open_session().await.unwrap();
        channel
            .request_pty(true, "xterm", 40, 8, 0, 0, &[])
            .await
            .unwrap();
        assert!(matches!(message(&mut channel).await, ChannelMsg::Success));
        channel.request_shell(true).await.unwrap();
        output(&mut channel, b"\x1b[?1049h").await;
        assert_eq!(alive.load(Ordering::SeqCst), 1);
        running.abort();
        assert!(running.await.unwrap_err().is_cancelled());
        while alive.load(Ordering::SeqCst) != 0 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
}

/// A consumer panic must restore the remote terminal and close its channel.
struct Panicking;
impl App for Panicking {
    fn frame(&mut self, _: u16, _: u16) -> Result<&Buffer, Error> {
        panic!("consumer render failed");
    }
    fn event(&mut self, _: Event) -> Result<bool, Error> {
        Ok(true)
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn application_panic_restores_terminal_and_exits_with_failure() {
    let host = key();
    let host_public = host.public_key().clone();
    let allowed = Arc::new(key());
    let public = allowed.public_key().clone();
    let server = Server::new(host, move |_, key| key == &public, |_| Ok(Panicking));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (stop, stopped) = oneshot::channel();
    let running = tokio::spawn(server.serve(listener, async {
        let _ = stopped.await;
    }));
    let mut client = client::connect(
        Arc::new(client::Config::default()),
        address,
        Client(host_public),
    )
    .await
    .unwrap();
    assert!(client
        .authenticate_publickey("guest", PrivateKeyWithHashAlg::new(allowed, None))
        .await
        .unwrap()
        .success());
    let mut channel = client.channel_open_session().await.unwrap();
    channel
        .request_pty(true, "xterm", 40, 8, 0, 0, &[])
        .await
        .unwrap();
    assert!(matches!(message(&mut channel).await, ChannelMsg::Success));
    channel.request_shell(true).await.unwrap();
    let mut restored = false;
    let mut status = None;
    loop {
        match message(&mut channel).await {
            ChannelMsg::Data { data } => {
                restored |= data.windows(8).any(|bytes| bytes == b"\x1b[?1049l");
            }
            ChannelMsg::ExitStatus { exit_status } => {
                status = Some(exit_status);
            }
            ChannelMsg::Close => break,
            _ => {}
        }
    }
    assert!(restored);
    assert_eq!(status, Some(1));
    stop.send(()).unwrap();
    timeout(Duration::from_secs(5), running)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
}

#[test]
fn a_tree_finishes_the_session_on_ctrl_c() {
    let mut tree = Tree::new();
    assert!(App::event(&mut tree, wove::Key::Char('c').into()).unwrap());
    assert!(!App::event(&mut tree, ctrl('c')).unwrap());
}

/// Holds its first event until released, so input piles up behind a busy app.
/// Ctrl-C is an ordinary event here; `q` finishes the session.
struct Gate {
    tree: Tree,
    release: Option<std::sync::mpsc::Receiver<()>>,
    seen: mpsc::UnboundedSender<Event>,
}
impl App for Gate {
    fn frame(&mut self, width: u16, height: u16) -> Result<&Buffer, Error> {
        Ok(self.tree.frame(width, height)?)
    }
    fn event(&mut self, event: Event) -> Result<bool, Error> {
        self.seen.send(event.clone())?;
        if let Some(release) = self.release.take() {
            release.recv()?;
        }
        Ok(event != wove::Key::Char('q').into())
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_busy_app_receives_an_input_burst_and_ctrl_c_without_disconnecting() {
    timeout(Duration::from_secs(20), async {
        let host = key();
        let host_public = host.public_key().clone();
        let allowed = Arc::new(key());
        let public = allowed.public_key().clone();
        let (release, gate) = std::sync::mpsc::channel();
        let gate = std::sync::Mutex::new(Some(gate));
        let (seen, mut events) = mpsc::unbounded_channel();
        let server = Server::new(
            host,
            move |_, key| key == &public,
            move |_| {
                Ok(Gate {
                    tree: Tree::new(),
                    release: gate.lock().unwrap().take(),
                    seen: seen.clone(),
                })
            },
        );
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let running = tokio::spawn(server.serve(listener, std::future::pending()));
        let mut client = client::connect(
            Arc::new(client::Config::default()),
            address,
            Client(host_public),
        )
        .await
        .unwrap();
        assert!(client
            .authenticate_publickey("guest", PrivateKeyWithHashAlg::new(allowed, None))
            .await
            .unwrap()
            .success());
        let mut channel = client.channel_open_session().await.unwrap();
        channel
            .request_pty(true, "xterm", 40, 8, 0, 0, &[])
            .await
            .unwrap();
        assert!(matches!(message(&mut channel).await, ChannelMsg::Success));
        channel.request_shell(true).await.unwrap();
        output(&mut channel, b"\x1b[?1049h").await;
        // The app blocks on this first key while every later packet arrives.
        channel.data(&b"a"[..]).await.unwrap();
        assert_eq!(events.recv().await.unwrap(), wove::Key::Char('a').into());
        for _ in 0..40 {
            channel.data(&b"b"[..]).await.unwrap();
        }
        channel.window_change(50, 10, 0, 0).await.unwrap();
        channel.data(&b"\x03"[..]).await.unwrap();
        // The server answers requests in order, so a reply proves that the
        // connection survived all of the input above.
        channel.exec(true, "uname").await.unwrap();
        assert!(matches!(message(&mut channel).await, ChannelMsg::Failure));
        release.send(()).unwrap();
        let mut received = Vec::new();
        while received.last() != Some(&ctrl('c')) {
            received.push(events.recv().await.unwrap());
        }
        let b: Event = wove::Key::Char('b').into();
        assert_eq!(received.iter().filter(|event| **event == b).count(), 40);
        assert!(received.contains(&Event::Resize(50, 10)));
        channel.data(&b"q"[..]).await.unwrap();
        output(&mut channel, b"\x1b[?1049l").await;
        running.abort();
    })
    .await
    .unwrap();
}

/// Shows the latest number another thread sent, taking it when asked for a frame.
struct Ticker {
    tree: Tree,
    text: wove::Id,
    ticks: std::sync::mpsc::Receiver<u32>,
}
impl App for Ticker {
    fn frame(&mut self, width: u16, height: u16) -> Result<&Buffer, Error> {
        for tick in self.ticks.try_iter() {
            self.tree
                .update::<Text>(self.text, |text| text.content = format!("tick {tick}"))?;
        }
        Ok(self.tree.frame(width, height)?)
    }
    fn event(&mut self, _: Event) -> Result<bool, Error> {
        Ok(true)
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_waker_draws_background_work_without_waiting_for_input() {
    timeout(Duration::from_secs(20), async {
        let host = key();
        let host_public = host.public_key().clone();
        let allowed = Arc::new(key());
        let public = allowed.public_key().clone();
        let server = Server::new(
            host,
            move |_, key| key == &public,
            |peer| {
                let mut tree = Tree::new();
                let text = tree.add(tree.root(), Text::new("waiting"))?;
                let (send, ticks) = std::sync::mpsc::channel();
                let waker = peer.waker.clone();
                std::thread::spawn(move || {
                    // Arrive after the first frame, while the app is idle.
                    std::thread::sleep(Duration::from_millis(100));
                    send.send(7).unwrap();
                    waker.wake();
                });
                Ok(Ticker { tree, text, ticks })
            },
        );
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let running = tokio::spawn(server.serve(listener, std::future::pending()));
        let mut client = client::connect(
            Arc::new(client::Config::default()),
            address,
            Client(host_public),
        )
        .await
        .unwrap();
        assert!(client
            .authenticate_publickey("guest", PrivateKeyWithHashAlg::new(allowed, None))
            .await
            .unwrap()
            .success());
        let mut channel = client.channel_open_session().await.unwrap();
        channel
            .request_pty(true, "xterm", 40, 8, 0, 0, &[])
            .await
            .unwrap();
        assert!(matches!(message(&mut channel).await, ChannelMsg::Success));
        channel.request_shell(true).await.unwrap();
        output(&mut channel, b"waiting").await;
        output(&mut channel, b"tick 7").await;
        running.abort();
    })
    .await
    .unwrap();
}
