//! A game and the players at it, shared by every session that shows it, and
//! the worker threads that play or analyse it. Sessions hold the room behind
//! a mutex; a change to it calls each session's watcher, which wakes that
//! session's loop so it draws again.
use crate::robot;
use crate::rules::{Color, Game, Move, Outcome};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

/// How long the analysis behind the evaluation bar thinks about a position.
const ANALYSIS: Duration = Duration::from_millis(350);
/// How long the robot thinks about its move.
const THINKING: Duration = Duration::from_secs(1);

/// A game and the players seated at it.
pub struct Room {
    pub game: Game,
    /// The white and black seats.
    pub seats: [Option<Player>; 2],
    /// Called when the room changes, so that each session draws it again.
    pub watchers: Vec<(u64, Box<dyn Fn() + Send>)>,
    /// The engine's view of the position, from White's side, once it has
    /// one. Only a game against the robot shows it: between two people the
    /// engine's opinion would spoil the game.
    pub engine: bool,
    pub eval: Option<i32>,
    analysing: bool,
    /// The robot is choosing its move.
    pub thinking: bool,
}

pub struct Player {
    pub id: u64,
    pub name: String,
}

impl Room {
    pub fn new() -> Self {
        Self {
            game: Game::new(),
            seats: [None, None],
            watchers: Vec::new(),
            engine: false,
            eval: None,
            analysing: false,
            thinking: false,
        }
    }

    /// Take the first free seat.
    pub fn join(&mut self, id: u64, name: &str) -> Option<Color> {
        let seat = self.seats.iter().position(Option::is_none)?;
        self.seats[seat] = Some(Player {
            id,
            name: name.into(),
        });
        self.changed();
        Some([Color::White, Color::Black][seat])
    }

    pub fn leave(&mut self, id: u64) {
        for seat in &mut self.seats {
            if seat.as_ref().is_some_and(|player| player.id == id) {
                *seat = None;
            }
        }
        self.watchers.retain(|(watcher, _)| *watcher != id);
        self.changed();
    }

    pub fn player(&self, color: Color) -> Option<&Player> {
        self.seats[usize::from(color == Color::Black)].as_ref()
    }

    /// Play a move into the game; the position's evaluation starts over.
    pub fn play(&mut self, mv: Move) -> bool {
        let played = self.game.play(mv).is_ok();
        if played {
            self.eval = None;
            self.changed();
        }
        played
    }

    pub fn restart(&mut self) {
        self.game = Game::new();
        self.eval = None;
        self.changed();
    }

    pub fn changed(&mut self) {
        for (_, wake) in &self.watchers {
            wake();
        }
    }
}

/// Evaluate the room's position on a worker thread unless that is under way
/// or done. The result reaches every session through the room's watchers.
pub fn analyse(shared: &Arc<Mutex<Room>>) {
    let mut room = lock(shared);
    if !room.engine || room.eval.is_some() || room.analysing {
        return;
    }
    // A finished game needs no engine: the bar goes all the way, or halfway.
    if let Some(outcome) = room.game.outcome {
        room.eval = Some(match outcome {
            Outcome::Checkmate(Color::White) => robot::MATE,
            Outcome::Checkmate(Color::Black) => -robot::MATE,
            _ => 0,
        });
        return;
    }
    room.analysing = true;
    let position = room.game.position.clone();
    drop(room);
    let shared = shared.clone();
    std::thread::spawn(move || {
        let score = robot::analyse(&position, ANALYSIS);
        let mut room = lock(&shared);
        room.analysing = false;
        if room.game.position == position {
            room.eval = Some(score);
            room.changed();
        }
    });
}

/// Plays one seat from a worker thread, so the terminal keeps taking input
/// while it thinks. Its move goes into the room like any other and wakes the
/// sessions through the room's watchers.
pub struct Robot {
    pub room: Arc<Mutex<Room>>,
    pub color: Color,
}

impl Robot {
    /// Start thinking when it is the robot's move and it is not already.
    pub fn prompt(&self) {
        let mut room = lock(&self.room);
        if room.game.outcome.is_some() || room.game.position.turn != self.color || room.thinking {
            return;
        }
        room.thinking = true;
        room.changed();
        let position = room.game.position.clone();
        drop(room);
        let shared = self.room.clone();
        std::thread::spawn(move || {
            let chosen = robot::choose(&position, THINKING);
            let mut room = lock(&shared);
            room.thinking = false;
            // A new game may have begun meanwhile; a move for the old one is dropped.
            match chosen {
                Some(mv) if room.game.position == position => {
                    room.play(mv);
                }
                _ => room.changed(),
            }
        });
    }
}

/// The rooms a server keeps. A player joins the first room with a free seat
/// or opens a new one, so games form in the order players arrive.
#[derive(Default)]
pub struct Lobby {
    rooms: Vec<Arc<Mutex<Room>>>,
    next: u64,
}

impl Lobby {
    pub fn join(&mut self, name: &str) -> (Arc<Mutex<Room>>, u64, Color) {
        self.next += 1;
        let id = self.next;
        for room in &self.rooms {
            if let Some(color) = lock(room).join(id, name) {
                return (room.clone(), id, color);
            }
        }
        let room = Arc::new(Mutex::new(Room::new()));
        let color = lock(&room).join(id, name).expect("a new room has seats");
        self.rooms.push(room.clone());
        (room, id, color)
    }

    pub fn leave(&mut self, room: &Arc<Mutex<Room>>, id: u64) {
        let empty = {
            let mut room = lock(room);
            room.leave(id);
            room.seats.iter().all(Option::is_none)
        };
        if empty {
            self.rooms.retain(|other| !Arc::ptr_eq(other, room));
        }
    }
}

/// A poisoned lock still holds a usable room; a panicking session must not
/// take the game away from the others.
pub fn lock<T>(shared: &Mutex<T>) -> MutexGuard<'_, T> {
    shared.lock().unwrap_or_else(PoisonError::into_inner)
}
