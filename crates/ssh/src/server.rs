use crate::runtime::{run, Message};
use crate::{Error, PrivateKey, PublicKey};
use russh::{
    server::{self, Auth, Msg, Session},
    Channel, ChannelId,
};
use std::{
    future::Future,
    net::{Shutdown, SocketAddr, TcpStream},
    sync::Arc,
    time::Duration,
};
use tokio::{
    net::TcpListener,
    sync::{mpsc, watch},
    task::JoinSet,
};
use wove::{Buffer, Event, Tree};

/// A remote application owns its state on one worker thread. It need not be Send.
pub trait App {
    /// Render the current application into a frame of the requested dimensions.
    fn frame(&mut self, width: u16, height: u16) -> Result<&Buffer, Error>;
    /// Return false to finish the SSH session after this event.
    fn event(&mut self, event: Event) -> Result<bool, Error>;
}
impl App for Tree {
    fn frame(&mut self, width: u16, height: u16) -> Result<&Buffer, Error> {
        Ok(Tree::frame(self, width, height)?)
    }
    fn event(&mut self, event: Event) -> Result<bool, Error> {
        self.dispatch(event)?;
        Ok(true)
    }
}

/// Identity and PTY metadata verified before constructing the application.
#[derive(Clone, Debug)]
pub struct Peer {
    pub user: String,
    pub key: PublicKey,
    pub address: SocketAddr,
    pub term: String,
    pub width: u16,
    pub height: u16,
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
        }
    }
    /// Serve an already-bound listener until shutdown. Each connection permits one
    /// interactive channel; closing the server cancels connections and their workers.
    pub async fn serve(
        self,
        listener: TcpListener,
        shutdown: impl Future<Output = ()>,
    ) -> Result<(), Error> {
        let config = Arc::new(server::Config {
            keys: vec![self.key],
            inactivity_timeout: Some(Duration::from_secs(300)),
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
                    let (stream,address)=incoming?;
                    let stream = stream.into_std()?;
                    let socket = Socket(stream.try_clone()?);
                    let stream = tokio::net::TcpStream::from_std(stream)?;
                    let client=Client{authorize:self.authorize.clone(),factory:self.factory.clone(),address,identity:None,channel:None,pty:None,input:None};
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
    pty: Option<(String, u16, u16)>,
    input: Option<mpsc::Sender<Message>>,
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
        {
            session.channel_failure(channel)?;
            return Ok(());
        }
        let (user, key) = self.identity.clone().unwrap();
        let (term, width, height) = self.pty.clone().unwrap();
        let peer = Peer {
            user,
            key,
            address: self.address,
            term,
            width,
            height,
        };
        let (sender, receiver) = mpsc::channel(16);
        self.input = Some(sender);
        let factory = self.factory.clone();
        let handle = session.handle();
        session.channel_success(channel)?;
        tokio::task::spawn_blocking(move || run(factory, peer, receiver, handle, channel));
        Ok(())
    }
    async fn data(
        &mut self,
        channel: ChannelId,
        data: &[u8],
        _: &mut Session,
    ) -> Result<(), Error> {
        if self.channel == Some(channel) {
            if data.len() > 65536 {
                return Err("SSH input packet exceeds limit".into());
            }
            if let Some(input) = &self.input {
                input
                    .try_send(Message::Data(data.to_vec()))
                    .map_err(|_| "application closed")?;
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
        if self.channel == Some(channel) {
            let (width, height) =
                dimensions(width, height).ok_or("invalid SSH terminal dimensions")?;
            if let Some(input) = &self.input {
                input
                    .try_send(Message::Resize(width, height))
                    .map_err(|_| "application closed")?;
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
            self.input.take();
            session.close(channel)?;
        }
        Ok(())
    }
    async fn channel_close(&mut self, channel: ChannelId, _: &mut Session) -> Result<(), Error> {
        if self.channel == Some(channel) {
            self.input.take();
        }
        Ok(())
    }
}

/// Close the transport even when its supervising future is dropped or cancelled.
struct Socket(TcpStream);
impl Drop for Socket {
    fn drop(&mut self) {
        let _ = self.0.shutdown(Shutdown::Both);
    }
}
