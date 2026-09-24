use crate::runtime::{run, Inbox, Waker};
use crate::{Error, PrivateKey, PublicKey};
use russh::{
    server::{self, Auth, Msg, Session},
    Channel, ChannelId, ChannelWriteHalf,
};
use std::{
    future::Future,
    io,
    net::{Shutdown, SocketAddr, TcpStream},
    sync::Arc,
    time::Duration,
};
use tokio::{net::TcpListener, sync::watch, task::JoinSet};
use wove::{Buffer, Event, Key, Modifiers, Tree};

/// A remote application owns its state on one worker thread. It need not be Send.
pub trait App {
    /// Render the current application into a frame of the requested dimensions.
    /// It is also called after `Peer::waker` wakes the session, so take
    /// work that arrived from other threads here.
    fn frame(&mut self, width: u16, height: u16) -> Result<&Buffer, Error>;
    /// Handle one event, including Ctrl-C, which the transport does not reserve.
    /// Return false to finish the SSH session after this event.
    fn event(&mut self, event: Event) -> Result<bool, Error>;
}
/// A tree dispatches every event to its focused node, except Ctrl-C, which
/// finishes the session.
impl App for Tree {
    fn frame(&mut self, width: u16, height: u16) -> Result<&Buffer, Error> {
        Ok(Tree::frame(self, width, height)?)
    }
    fn event(&mut self, event: Event) -> Result<bool, Error> {
        if matches!(
            event,
            Event::Key(Key::Char('c'), Modifiers { ctrl: true, .. })
        ) {
            return Ok(false);
        }
        self.dispatch(event)?;
        Ok(true)
    }
}

/// Identity and PTY metadata verified before constructing the application,
/// and the waker for the application's thread.
#[derive(Clone, Debug)]
pub struct Peer {
    pub user: String,
    pub key: PublicKey,
    pub address: SocketAddr,
    pub term: String,
    pub width: u16,
    pub height: u16,
    /// Hand this to background work that should reach the peer while the
    /// application is idle.
    pub waker: Waker,
}

type Authorize = Arc<dyn Fn(&str, &PublicKey) -> bool + Send + Sync>;
pub(crate) type Factory = Arc<dyn Fn(&Peer) -> Result<Box<dyn App>, Error> + Send + Sync>;

/// An SSH application server. Public-key authentication and a PTY are required.
/// Shell commands, forwarding, and subsystems are not exposed.
pub struct Server {
    key: PrivateKey,
    authorize: Authorize,
    factory: Factory,
    /// Maximum simultaneous SSH connections, including pending authentication.
    pub connections: usize,
    /// Close a connection after this long without traffic in either direction;
    /// `None` keeps idle connections open. Defaults to five minutes.
    pub inactivity: Option<Duration>,
}
impl Server {
    /// Use a persistent host key, a public-key policy, and one app factory per peer.
    pub fn new<A: App + 'static>(
        key: PrivateKey,
        authorize: impl Fn(&str, &PublicKey) -> bool + Send + Sync + 'static,
        factory: impl Fn(&Peer) -> Result<A, Error> + Send + Sync + 'static,
    ) -> Self {
        Self {
            key,
            authorize: Arc::new(authorize),
            factory: Arc::new(move |peer| Ok(Box::new(factory(peer)?))),
            connections: 32,
            inactivity: Some(Duration::from_secs(300)),
        }
    }
    /// Serve an already-bound listener until shutdown. Each connection permits one
    /// interactive channel; closing the server cancels connections and their workers.
    /// A failed accept, such as when file descriptors run out, pauses accepting
    /// briefly instead of ending the server.
    pub async fn serve(
        self,
        listener: TcpListener,
        shutdown: impl Future<Output = ()>,
    ) -> Result<(), Error> {
        let config = Arc::new(server::Config {
            keys: vec![self.key],
            inactivity_timeout: self.inactivity,
            auth_rejection_time: Duration::from_millis(250),
            ..Default::default()
        });
        let mut tasks = JoinSet::new();
        let (stop, _) = watch::channel(false);
        tokio::pin!(shutdown);
        loop {
            tokio::select! {
                _=&mut shutdown=>break,
                Some(_)=tasks.join_next(),if !tasks.is_empty()=>{},
                incoming=listener.accept(),if tasks.len()<self.connections.max(1)=>{
                    let Ok((stream, socket, address)) = incoming.and_then(|(stream, address)| {
                        let (stream, socket) = supervise(stream)?;
                        Ok((stream, socket, address))
                    }) else {
                        tokio::time::sleep(Duration::from_millis(100)).await;
                        continue;
                    };
                    let client=Client{authorize:self.authorize.clone(),factory:self.factory.clone(),address,identity:None,channel:None,output:None,pty:None,input:None};
                    let config=config.clone();
                    let mut stopping = stop.subscribe();
                    tasks.spawn(async move {
                        let _socket = socket;
                        let opening = tokio::time::timeout(Duration::from_secs(10), server::run_stream(config,stream,client));
                        let session = tokio::select! {
                            _ = stopping.changed() => return,
                            result = opening => match result { Ok(Ok(session)) => session, _ => return },
                        };
                        let handle = session.handle();
                        tokio::pin!(session);
                        tokio::select! {
                            _ = &mut session => {},
                            _ = stopping.changed() => {
                                let _ = tokio::time::timeout(Duration::from_secs(5), async {
                                    let _ = handle.disconnect(russh::Disconnect::ByApplication, "server stopped".into(), String::new()).await;
                                    let _ = session.await;
                                }).await;
                            }
                        }
                    });
                }
            }
        }
        let _ = stop.send(true);
        while tasks.join_next().await.is_some() {}
        Ok(())
    }
}

struct Client {
    authorize: Authorize,
    factory: Factory,
    address: SocketAddr,
    identity: Option<(String, PublicKey)>,
    channel: Option<ChannelId>,
    /// The window-aware write half, handed to the application at shell start.
    output: Option<ChannelWriteHalf<Msg>>,
    pty: Option<(String, u16, u16)>,
    input: Option<Arc<Inbox>>,
}
impl Drop for Client {
    /// A connection that ends without closing its channel still ends its app.
    fn drop(&mut self) {
        if let Some(input) = self.input.take() {
            input.close();
        }
    }
}

/// Reject oversized or empty grids before allocating frame storage.
fn dimensions(width: u32, height: u32) -> Option<(u16, u16)> {
    if width == 0 || height == 0 || width > 512 || height > 256 || width * height > 65536 {
        None
    } else {
        Some((width as u16, height as u16))
    }
}
impl server::Handler for Client {
    type Error = Error;
    async fn auth_publickey(&mut self, user: &str, key: &PublicKey) -> Result<Auth, Error> {
        if (self.authorize)(user, key) {
            self.identity = Some((user.into(), key.clone()));
            Ok(Auth::Accept)
        } else {
            Ok(Auth::reject())
        }
    }
    async fn channel_open_session(
        &mut self,
        channel: Channel<Msg>,
        reply: server::ChannelOpenHandle,
        _: &mut Session,
    ) -> Result<(), Error> {
        if self.channel.is_none() && self.identity.is_some() {
            self.channel = Some(channel.id());
            // Input arrives through the handler callbacks. Dropping the read half
            // keeps russh from waiting on a queue that nothing drains.
            let (_, output) = channel.split();
            self.output = Some(output);
            reply.accept().await;
        } else {
            reply
                .reject(russh::ChannelOpenFailure::AdministrativelyProhibited)
                .await;
        }
        Ok(())
    }
    async fn pty_request(
        &mut self,
        channel: ChannelId,
        term: &str,
        width: u32,
        height: u32,
        _: u32,
        _: u32,
        _: &[(russh::Pty, u32)],
        session: &mut Session,
    ) -> Result<(), Error> {
        if self.channel == Some(channel) && self.input.is_none() && term.len() <= 128 {
            if let Some((width, height)) = dimensions(width, height) {
                self.pty = Some((term.into(), width, height));
                session.channel_success(channel)?;
                return Ok(());
            }
        }
        session.channel_failure(channel)?;
        Ok(())
    }
    async fn exec_request(
        &mut self,
        channel: ChannelId,
        _: &[u8],
        session: &mut Session,
    ) -> Result<(), Error> {
        session.channel_failure(channel)?;
        Ok(())
    }
    async fn subsystem_request(
        &mut self,
        channel: ChannelId,
        _: &str,
        session: &mut Session,
    ) -> Result<(), Error> {
        session.channel_failure(channel)?;
        Ok(())
    }
    async fn env_request(
        &mut self,
        channel: ChannelId,
        _: &str,
        _: &str,
        session: &mut Session,
    ) -> Result<(), Error> {
        session.channel_failure(channel)?;
        Ok(())
    }
    async fn shell_request(
        &mut self,
        channel: ChannelId,
        session: &mut Session,
    ) -> Result<(), Error> {
        if self.channel != Some(channel)
            || self.input.is_some()
            || self.pty.is_none()
            || self.identity.is_none()
            || self.output.is_none()
        {
            session.channel_failure(channel)?;
            return Ok(());
        }
        let (user, key) = self.identity.clone().unwrap();
        let (term, width, height) = self.pty.clone().unwrap();
        let input = Arc::new(Inbox::default());
        let peer = Peer {
            user,
            key,
            address: self.address,
            term,
            width,
            height,
            waker: Waker::new(&input),
        };
        self.input = Some(input.clone());
        let output = self.output.take().unwrap();
        let factory = self.factory.clone();
        session.channel_success(channel)?;
        tokio::task::spawn_blocking(move || run(factory, peer, input, output));
        Ok(())
    }
    async fn data(
        &mut self,
        channel: ChannelId,
        data: &[u8],
        _: &mut Session,
    ) -> Result<(), Error> {
        if self.channel == Some(channel) {
            if let Some(input) = &self.input {
                input.data(data)?;
            }
        }
        Ok(())
    }
    async fn window_change_request(
        &mut self,
        channel: ChannelId,
        width: u32,
        height: u32,
        _: u32,
        _: u32,
        _: &mut Session,
    ) -> Result<(), Error> {
        // A window the peer reports as empty or absurd keeps the last size;
        // it is not worth the session.
        if let (true, Some((width, height))) =
            (self.channel == Some(channel), dimensions(width, height))
        {
            if let Some(input) = &self.input {
                input.resize(width, height);
            } else if let Some(pty) = &mut self.pty {
                pty.1 = width;
                pty.2 = height;
            }
        }
        Ok(())
    }
    async fn channel_eof(
        &mut self,
        channel: ChannelId,
        session: &mut Session,
    ) -> Result<(), Error> {
        if self.channel == Some(channel) {
            if let Some(input) = self.input.take() {
                input.close();
            }
            session.close(channel)?;
        }
        Ok(())
    }
    async fn channel_close(&mut self, channel: ChannelId, _: &mut Session) -> Result<(), Error> {
        if self.channel == Some(channel) {
            if let Some(input) = self.input.take() {
                input.close();
            }
        }
        Ok(())
    }
}

/// Split an accepted stream into the one russh drives and a duplicate that
/// shuts the socket down when the connection task ends.
fn supervise(stream: tokio::net::TcpStream) -> io::Result<(tokio::net::TcpStream, Socket)> {
    let stream = stream.into_std()?;
    let socket = Socket(stream.try_clone()?);
    Ok((tokio::net::TcpStream::from_std(stream)?, socket))
}

/// Close the transport even when its supervising future is dropped or cancelled.
struct Socket(TcpStream);
impl Drop for Socket {
    fn drop(&mut self) {
        let _ = self.0.shutdown(Shutdown::Both);
    }
}
