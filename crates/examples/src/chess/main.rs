//! Chess in the terminal, against a robot, for two at one keyboard,
//! or over SSH. Every player's screen shows a shared game, and a move by one
//! side, or by the robot's worker thread, wakes the other so it is drawn at once.
//!
//! ```sh
//! cargo run -p wove-examples --release --bin chess -- robot   # you are White
//! cargo run -p wove-examples --bin chess                       # two players, one terminal
//! ssh-keygen -t ed25519 -N '' -f chess-host-key                # once, for the server
//! cargo run -p wove-examples --bin chess -- serve chess-host-key
//! ssh -p 2222 alice@127.0.0.1                                   # any key is accepted
//! ```
mod board;
mod pieces;
mod robot;
mod room;
mod rules;
mod session;

use room::{lock, Lobby, Robot, Room};
use rules::Color;
use session::{Seat, Session};
use std::sync::{Arc, Mutex};
use wove::{
    elements::Blocks,
    terminal::{self, Terminal},
    Event, Key, Modifiers, Options, Tree,
};

/// Sextants for terminals known to draw them without a font's help; half
/// blocks, which every terminal draws, for the rest. The `p` key overrides.
fn blocks_for(terminal: &str) -> Blocks {
    let terminal = terminal.to_lowercase();
    let native = ["ghostty", "wezterm", "kitty", "foot", "contour"];
    if native.iter().any(|name| terminal.contains(name)) {
        Blocks::Sextant
    } else {
        Blocks::Half
    }
}

/// Play at this terminal, with the robot in the other seat when there is one.
/// The loop draws after every event and after every wake, so a robot move
/// made while the user sits still, or the next frame of a gliding piece, is
/// shown as soon as it is ready.
fn local(room: Arc<Mutex<Room>>, seat: Seat, robot: Option<Robot>) -> Result<(), wove_ssh::Error> {
    let waker = terminal::waker()?;
    let wake: Arc<dyn Fn() + Send + Sync> = Arc::new(move || waker.wake());
    // Watch the room before the session first reads it, so nothing that
    // changes in between goes undrawn.
    let watcher = wake.clone();
    lock(&room).watchers.push((0, Box::new(move || watcher())));
    let term = std::env::var("TERM").unwrap_or_default();
    let program = std::env::var("TERM_PROGRAM").unwrap_or_default();
    let blocks = blocks_for(&format!("{term} {program}"));
    let mut tree = Tree::new();
    let session = Session::new(&mut tree, room, seat, wake, "esc", blocks)?;
    // Pointer motion lets the board show the square under the mouse.
    let mut terminal = Terminal::with_options(Options {
        motion: true,
        ..Options::default()
    })?;
    let mut typed = terminal.typed_ahead().into_iter();
    loop {
        session.sync(&mut tree)?;
        if let Some(robot) = &robot {
            robot.prompt();
        }
        let (width, height) = terminal.size()?;
        session.fit(&mut tree, width, height)?;
        terminal.draw(tree.frame(width, height)?)?;
        let event = match typed.next() {
            Some(event) => Some(event),
            None => terminal::read()?,
        };
        let Some(event) = event else {
            continue;
        };
        if let Event::Resize(..) = event {
            terminal.invalidate();
        }
        let dispatch = tree.dispatch(event.clone())?;
        session.after(&mut tree, &event, &dispatch)?;
        if event == Key::Escape.into() && !dispatch.handled {
            return Ok(());
        }
    }
}

/// Two players at one keyboard.
fn hotseat() -> Result<(), wove_ssh::Error> {
    let room = Arc::new(Mutex::new(Room::new()));
    {
        let mut room = lock(&room);
        room.join(1, "Player 1");
        room.join(2, "Player 2");
    }
    local(room, Seat::Both, None)
}

/// One player against the robot, which takes the other seat.
fn versus_robot(you: Color) -> Result<(), wove_ssh::Error> {
    let room = Arc::new(Mutex::new(Room::new()));
    {
        let mut room = lock(&room);
        room.engine = true;
        let names = if you == Color::White {
            ["You", "Robot"]
        } else {
            ["Robot", "You"]
        };
        for (id, name) in names.into_iter().enumerate() {
            room.join(id as u64 + 1, name);
        }
    }
    let robot = Robot {
        room: room.clone(),
        color: you.other(),
    };
    local(room, Seat::One(you), Some(robot))
}

/// One SSH connection: its tree, its seat, and its place in the lobby.
struct Remote {
    tree: Tree,
    session: Session,
    lobby: Arc<Mutex<Lobby>>,
    id: u64,
}

impl Remote {
    fn new(lobby: Arc<Mutex<Lobby>>, peer: &wove_ssh::Peer) -> Result<Self, wove_ssh::Error> {
        let (room, id, color) = lock(&lobby).join(&peer.user);
        let waker = peer.waker.clone();
        let wake: Arc<dyn Fn() + Send + Sync> = Arc::new(move || waker.wake());
        let watcher = wake.clone();
        lock(&room).watchers.push((id, Box::new(move || watcher())));
        let mut tree = Tree::new();
        let blocks = blocks_for(&peer.term);
        let session = Session::new(&mut tree, room, Seat::One(color), wake, "ctrl-c", blocks)?;
        Ok(Self {
            tree,
            session,
            lobby,
            id,
        })
    }
}

impl Drop for Remote {
    fn drop(&mut self) {
        lock(&self.lobby).leave(&self.session.room, self.id);
    }
}

impl wove_ssh::App for Remote {
    fn frame(&mut self, width: u16, height: u16) -> Result<&wove::Buffer, wove_ssh::Error> {
        self.session.fit(&mut self.tree, width, height)?;
        self.session.sync(&mut self.tree)?;
        Ok(self.tree.frame(width, height)?)
    }

    fn event(&mut self, event: Event) -> Result<bool, wove_ssh::Error> {
        if let Event::Key(Key::Char('c' | 'd'), Modifiers { ctrl: true, .. }) = event {
            return Ok(false);
        }
        let dispatch = self.tree.dispatch(event.clone())?;
        self.session.after(&mut self.tree, &event, &dispatch)?;
        Ok(true)
    }
}

/// Serve games to SSH players. Any public key is accepted; the SSH user name
/// is the player's name at the board.
async fn serve(key: &str, address: &str) -> Result<(), wove_ssh::Error> {
    let key = wove_ssh::PrivateKey::read_openssh_file(std::path::Path::new(key))?;
    let lobby = Arc::new(Mutex::new(Lobby::default()));
    let server = wove_ssh::Server::new(
        key,
        |_, _| true,
        move |peer| Remote::new(lobby.clone(), peer),
    );
    let listener = tokio::net::TcpListener::bind(address).await?;
    let address = listener.local_addr()?;
    eprintln!(
        "chess is listening on {address}. Play with: ssh -p {} NAME@{}",
        address.port(),
        address.ip()
    );
    server
        .serve(listener, async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await
}

fn main() -> Result<(), wove_ssh::Error> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        [] => hotseat(),
        ["robot"] => versus_robot(Color::White),
        ["robot", "black"] => versus_robot(Color::Black),
        ["serve", key] => tokio::runtime::Runtime::new()?.block_on(serve(key, "127.0.0.1:2222")),
        ["serve", key, address] => tokio::runtime::Runtime::new()?.block_on(serve(key, address)),
        _ => Err("usage: chess [robot [black] | serve <host-key> [address]]".into()),
    }
}
