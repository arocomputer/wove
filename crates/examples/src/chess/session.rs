//! What one terminal shows of a room: the board, the cards beside it, the
//! key hints beneath, and the seat the terminal plays. A session owns the
//! node ids in a tree it is handed, fits the layout to the terminal, keeps
//! the tree in step with the room, and turns the keys the board leaves
//! alone into room changes.
use crate::board::{Board, THEMES};
use crate::robot;
use crate::room::{analyse, lock, Room};
use crate::rules::{Color, Game, Kind, Square};
use std::cell::Cell;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use wove::{
    elements::{Blocks, Container, Panel, RichText, Scroll, Text},
    layout::*,
    text::{Span, Wrap},
    Border, Canvas, Dispatch, Element, Event, Id, Key, Style, Tree,
};

/// Whose moves a terminal makes: both sides at one keyboard, or one seat.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Seat {
    Both,
    One(Color),
}

/// The side card's width, which the board's size is chosen around.
const SIDE: u16 = 32;
/// The gap between the eval bar, the board, and the side card.
const GAP: u16 = 2;
/// The rows the logo takes when it shows.
const LOGO: u16 = 3;

const ACCENT: wove::Color = wove::Color::Rgb(255, 214, 92);

fn dim() -> Style {
    Style {
        fg: wove::Color::Rgb(128, 128, 136),
        ..Style::default()
    }
}

fn bold() -> Style {
    Style {
        bold: true,
        ..Style::default()
    }
}

/// A column or row whose children sit in its middle, but never past its
/// start when they overflow.
fn centered() -> wove::Layout {
    wove::Layout {
        align_items: Some(AlignItems {
            keyword: AlignItemsKeyword::Center,
            safety: AlignmentSafety::Safe,
        }),
        justify_content: Some(JustifyContent {
            keyword: AlignContentKeyword::Center,
            safety: AlignmentSafety::Safe,
        }),
        ..Default::default()
    }
}

/// A rounded, dim-bordered card with a cell of padding at each side.
fn card(tree: &mut Tree, parent: Id) -> Result<Id, wove::Error> {
    let id = tree.add(
        parent,
        Panel {
            style: dim(),
            border: Border::Rounded,
            ..Default::default()
        },
    )?;
    tree.set_layout(
        id,
        wove::Layout {
            flex_shrink: 0.0,
            border: Rect::length(1.0),
            padding: Rect {
                left: length(1.0),
                right: length(1.0),
                top: length(0.0),
                bottom: length(0.0),
            },
            ..Default::default()
        },
    )?;
    Ok(id)
}

/// The evaluation bar beside the board: White's share of the column grows
/// with the engine's score, and sits at the bottom unless the board is flipped.
struct Eval {
    score: Option<i32>,
    flipped: bool,
}

impl Element for Eval {
    fn measure(&self, _width: Option<u16>) -> (u16, u16) {
        (1, 16)
    }

    fn paint(&self, canvas: &mut Canvas<'_>) {
        let rows = canvas.size().1;
        let share = match self.score {
            None => 0.5,
            Some(score) if score >= robot::MATE - 100 => 1.0,
            Some(score) if score <= 100 - robot::MATE => 0.0,
            Some(score) => 1.0 / (1.0 + 10f32.powf(-(score as f32) / 400.0)),
        };
        let pixels = 2 * usize::from(rows);
        let white = (share * pixels as f32).round() as usize;
        let (light, dark) = (
            wove::Color::Rgb(232, 232, 228),
            wove::Color::Rgb(40, 40, 44),
        );
        let color = |pixel: usize| {
            let white_here = if self.flipped {
                pixel < white
            } else {
                pixel >= pixels - white
            };
            if white_here {
                light
            } else {
                dark
            }
        };
        for row in 0..usize::from(rows) {
            let (upper, lower) = (color(2 * row), color(2 * row + 1));
            let (glyph, fg) = if upper == lower {
                (" ", lower)
            } else {
                ("▀", upper)
            };
            canvas.text(
                0,
                row as i32,
                glyph,
                Style {
                    fg,
                    bg: lower,
                    ..Style::default()
                },
            );
        }
    }
}

/// The nodes of the side card.
struct Side {
    /// The card above the score sheet, for the side at the top of the board,
    /// and the one below it.
    top: Id,
    bottom: Id,
    moves: Id,
    scroll: Id,
    status: Id,
}

pub struct Session {
    pub room: Arc<Mutex<Room>>,
    seat: Seat,
    logo: Id,
    subtitle: Id,
    body: Id,
    board: Id,
    /// The evaluation bar, in a game against the robot.
    eval: Option<Id>,
    side: Side,
    /// The terminal size the layout was last fitted to.
    fitted: Cell<(u16, u16)>,
    /// How wide the key hints are on one line.
    hints: u16,
    /// Asks the terminal loop for a frame; animation ticks come through it.
    wake: Arc<dyn Fn() + Send + Sync>,
    ticking: Arc<AtomicBool>,
    started: Instant,
}

impl Session {
    /// Build the screen in `tree`: a header, the board row, and the key
    /// card, centered as a column. `quit` names the key that leaves.
    pub fn new(
        tree: &mut Tree,
        room: Arc<Mutex<Room>>,
        seat: Seat,
        wake: Arc<dyn Fn() + Send + Sync>,
        quit: &'static str,
        blocks: Blocks,
    ) -> Result<Self, wove::Error> {
        let root = tree.root();
        tree.set_layout(
            root,
            wove::Layout {
                flex_direction: FlexDirection::Column,
                padding: Rect {
                    left: length(1.0),
                    right: length(1.0),
                    top: length(0.0),
                    bottom: length(0.0),
                },
                gap: Size {
                    width: length(0.0),
                    height: length(1.0),
                },
                ..centered()
            },
        )?;
        let (logo, subtitle) = header(tree, root)?;
        let viewer = match seat {
            Seat::One(Color::Black) => Color::Black,
            _ => Color::White,
        };
        let body = tree.add(root, Container)?;
        tree.set_layout(
            body,
            wove::Layout {
                flex_shrink: 0.0,
                gap: Size {
                    width: length(f32::from(GAP)),
                    height: length(0.0),
                },
                ..Default::default()
            },
        )?;
        let eval = if lock(&room).engine {
            Some(eval_bar(tree, body, viewer)?)
        } else {
            None
        };
        let mut board = Board::new(viewer);
        board.blocks = blocks;
        let board = tree.add(body, board)?;
        let side = side_card(tree, body)?;
        let hints = key_card(tree, root, quit)?;
        tree.focus(Some(board))?;
        let session = Self {
            room,
            seat,
            logo,
            subtitle,
            body,
            board,
            eval,
            side,
            fitted: Cell::new((0, 0)),
            hints,
            wake,
            ticking: Arc::new(AtomicBool::new(false)),
            started: Instant::now(),
        };
        session.sync(tree)?;
        Ok(session)
    }

    /// Size the board to the terminal: the largest squares that leave room
    /// for the side card beside it and the header and keys around it. The
    /// logo shows only when it costs the board nothing.
    pub fn fit(&self, tree: &mut Tree, width: u16, height: u16) -> Result<(), wove::Error> {
        if self.fitted.replace((width, height)) == (width, height) {
            return Ok(());
        }
        let bar = u16::from(self.eval.is_some()) * (1 + GAP);
        let columns = width.saturating_sub(2 + bar + GAP + SIDE);
        // The title, the key card, and the gaps between them. The card
        // wraps its hints into as many lines as the width makes it.
        let hint_lines = self.hints.div_ceil(width.saturating_sub(6).max(1));
        let chrome = 1 + (2 + hint_lines) + 2;
        let square = |rows: u16| (rows / 8).min(columns / 16);
        let mut rows = height.saturating_sub(chrome);
        let with_logo = square(rows.saturating_sub(LOGO));
        let show_logo = with_logo >= 3 && with_logo == square(rows);
        if show_logo {
            rows -= LOGO;
        }
        let side = square(rows).max(2);
        tree.set_layout(
            self.logo,
            wove::Layout {
                display: if show_logo {
                    Display::Flex
                } else {
                    Display::None
                },
                ..Default::default()
            },
        )?;
        // The name is in the logo when it shows, and in the line when it does not.
        let mut spans = vec![Span::new("chess on Wove", dim())];
        if !show_logo {
            spans.insert(
                0,
                Span::new(
                    "chess  ",
                    Style {
                        fg: ACCENT,
                        ..bold()
                    },
                ),
            );
        }
        tree.update::<RichText>(self.subtitle, |text| text.spans = spans)?;
        tree.set_layout(
            self.board,
            wove::Layout {
                flex_shrink: 0.0,
                size: Size {
                    width: length(f32::from(16 * side)),
                    height: length(f32::from(8 * side)),
                },
                ..Default::default()
            },
        )?;
        tree.set_layout(
            self.body,
            wove::Layout {
                flex_shrink: 0.0,
                size: Size {
                    width: auto(),
                    height: length(f32::from(8 * side)),
                },
                gap: Size {
                    width: length(f32::from(GAP)),
                    height: length(0.0),
                },
                ..Default::default()
            },
        )
    }

    /// Whether this terminal moves for `color`.
    fn plays(&self, color: Color) -> bool {
        self.seat == Seat::Both || self.seat == Seat::One(color)
    }

    /// Bring the tree up to date with the room and the clock. Cheap when
    /// nothing changed: the board compares positions, and text is replaced
    /// only when it differs.
    pub fn sync(&self, tree: &mut Tree) -> Result<(), wove::Error> {
        analyse(&self.room);
        let now = self.started.elapsed();
        let room = lock(&self.room);
        let game = &room.game;
        let active = game.outcome.is_none() && self.plays(game.position.turn);
        let banner = game.outcome.map(|outcome| {
            let (headline, detail) = outcome.headline();
            (
                headline.to_owned(),
                format!("{detail}  ·  n for a new game"),
            )
        });
        let mut moving = false;
        let mut flipped = false;
        let mut promoting = false;
        tree.update::<Board>(self.board, |board| {
            board.show(&game.position, game.last_move(), active);
            board.banner = banner;
            moving = board.tick(now);
            flipped = board.flipped;
            promoting = board.promoting.is_some();
        })?;
        if let Some(id) = self.eval {
            tree.update::<Eval>(id, |eval| {
                eval.score = room.eval;
                eval.flipped = flipped;
            })?;
        }
        let (upper, lower) = if flipped {
            (Color::White, Color::Black)
        } else {
            (Color::Black, Color::White)
        };
        let texts = [
            (self.side.top, player_card(&room, upper)),
            (self.side.bottom, player_card(&room, lower)),
            (self.side.moves, sheet(game)),
            (self.side.status, self.status(&room, promoting, now)),
        ];
        let busy = moving || room.thinking;
        drop(room);
        for (id, spans) in texts {
            if tree.get::<RichText>(id)?.spans != spans {
                tree.update::<RichText>(id, |text| text.spans = spans)?;
            }
        }
        if busy {
            self.tick_soon(if moving { 33 } else { 120 });
        }
        Ok(())
    }

    /// Ask for another frame in `millis`, unless one is already on its way.
    fn tick_soon(&self, millis: u64) {
        if self.ticking.swap(true, Ordering::SeqCst) {
            return;
        }
        let ticking = self.ticking.clone();
        let wake = self.wake.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(millis));
            ticking.store(false, Ordering::SeqCst);
            wake();
        });
    }

    /// The line under the players: whose move it is, or how the game ended,
    /// and the engine's score when there is one.
    fn status(&self, room: &Room, promoting: bool, now: Duration) -> Vec<Span> {
        let game = &room.game;
        let turn = game.position.turn;
        let mut spans = vec![
            Span::new(dot(turn), Style::default()),
            Span::new(" ", bold()),
        ];
        match game.outcome {
            Some(outcome) => spans.push(Span::new(outcome.describe(), bold())),
            None if promoting => {
                spans.push(Span::new("Promote: Q R B N, Enter for a queen", bold()))
            }
            None if room.thinking => {
                let frames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
                let frame = frames[(now.as_millis() / 100) as usize % frames.len()];
                spans.push(Span::new(format!("Robot is thinking {frame}"), bold()));
            }
            None => {
                let who = if self.seat == Seat::One(turn) {
                    "Your move".into()
                } else {
                    let name = room
                        .player(turn)
                        .map_or(turn.name(), |player| player.name.as_str());
                    format!("{name} to move")
                };
                spans.push(Span::new(who, bold()));
                if game.position.in_check() {
                    let red = Style {
                        fg: wove::Color::Rgb(230, 80, 60),
                        ..bold()
                    };
                    spans.push(Span::new("  check", red));
                }
            }
        }
        let eval = match room.eval {
            _ if !room.engine || game.outcome.is_some() => String::new(),
            None => "  …".into(),
            Some(score) if score.abs() >= robot::MATE - 100 => {
                let plies = robot::MATE - score.abs();
                let sign = if score > 0 { "+" } else { "-" };
                format!("  {sign}M{}", (plies + 1) / 2)
            }
            Some(score) => format!("  {:+.1}", score as f32 / 100.0),
        };
        spans.push(Span::new(eval, dim()));
        spans
    }

    /// After an event: play a chosen move into the room, handle the keys the
    /// board leaves alone, and draw whatever the room now holds.
    pub fn after(
        &self,
        tree: &mut Tree,
        event: &Event,
        dispatch: &Dispatch,
    ) -> Result<(), wove::Error> {
        let mut played = None;
        tree.update::<Board>(self.board, |board| played = board.played.take())?;
        if let Some(mv) = played {
            let mut room = lock(&self.room);
            if self.plays(room.game.position.turn) {
                room.play(mv);
            }
        }
        if !dispatch.handled {
            self.key(tree, event)?;
        }
        self.sync(tree)?;
        tree.update::<Scroll>(self.side.scroll, |scroll| scroll.follow = true)?;
        Ok(())
    }

    /// The keys that belong to the screen rather than the board.
    fn key(&self, tree: &mut Tree, event: &Event) -> Result<(), wove::Error> {
        let Event::Key(Key::Char(key), _) = event else {
            return Ok(());
        };
        match key {
            'f' => tree.update::<Board>(self.board, |board| board.flipped = !board.flipped),
            't' => tree.update::<Board>(self.board, |board| {
                board.theme = (board.theme + 1) % THEMES.len();
            }),
            'p' => tree.update::<Board>(self.board, |board| {
                board.blocks = match board.blocks {
                    Blocks::Sextant => Blocks::Half,
                    _ => Blocks::Sextant,
                };
            }),
            'n' => {
                let mut room = lock(&self.room);
                // Over SSH a game in progress is the other player's too.
                if room.game.outcome.is_some() || self.seat == Seat::Both {
                    room.restart();
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }
}

/// The logo and the line beneath it. The logo is hidden by `fit` when the
/// terminal is short.
fn header(tree: &mut Tree, root: Id) -> Result<(Id, Id), wove::Error> {
    let header = tree.add(root, Container)?;
    tree.set_layout(
        header,
        wove::Layout {
            flex_direction: FlexDirection::Column,
            flex_shrink: 0.0,
            ..centered()
        },
    )?;
    let logo = tree.add(
        header,
        Text {
            content: logo(),
            style: Style {
                fg: ACCENT,
                ..Style::default()
            },
            ..Text::default()
        },
    )?;
    let subtitle = tree.add(header, RichText::default())?;
    Ok((logo, subtitle))
}

fn eval_bar(tree: &mut Tree, body: Id, viewer: Color) -> Result<Id, wove::Error> {
    let eval = tree.add(
        body,
        Eval {
            score: None,
            flipped: viewer == Color::Black,
        },
    )?;
    tree.set_layout(
        eval,
        wove::Layout {
            flex_shrink: 0.0,
            size: Size {
                width: length(1.0),
                height: auto(),
            },
            ..Default::default()
        },
    )?;
    Ok(eval)
}

/// The card beside the board: a player, a rule, the score sheet, a rule,
/// the other player, and the status line.
fn side_card(tree: &mut Tree, body: Id) -> Result<Side, wove::Error> {
    let side = card(tree, body)?;
    let mut layout = tree.layout(side)?.clone();
    layout.flex_direction = FlexDirection::Column;
    layout.size.width = length(f32::from(SIDE));
    tree.set_layout(side, layout)?;
    let rule = |tree: &mut Tree| {
        tree.add(
            side,
            Text {
                content: "─".repeat(usize::from(SIDE) - 4),
                style: dim(),
                ..Text::default()
            },
        )
    };
    let top = tree.add(side, RichText::default())?;
    rule(tree)?;
    let scroll = tree.add(side, Scroll::default())?;
    tree.set_layout(
        scroll,
        wove::Layout {
            flex_grow: 1.0,
            min_size: Size {
                width: length(0.0),
                height: length(1.0),
            },
            ..Default::default()
        },
    )?;
    let moves = tree.add(scroll, RichText::default())?;
    tree.set_layout(
        moves,
        wove::Layout {
            flex_shrink: 0.0,
            ..Default::default()
        },
    )?;
    rule(tree)?;
    let bottom = tree.add(side, RichText::default())?;
    let status = tree.add(side, RichText::default())?;
    tree.set_layout(
        status,
        wove::Layout {
            margin: Rect {
                top: length(1.0),
                bottom: length(0.0),
                left: length(0.0),
                right: length(0.0),
            },
            ..Default::default()
        },
    )?;
    Ok(Side {
        top,
        bottom,
        moves,
        scroll,
        status,
    })
}

/// The key hints in a card under the board, wrapping when the terminal is
/// narrow. Returns how wide the hints are on one line.
fn key_card(tree: &mut Tree, root: Id, quit: &'static str) -> Result<u16, wove::Error> {
    let keys = card(tree, root)?;
    let mut layout = tree.layout(keys)?.clone();
    layout.max_size.width = percent(1.0);
    tree.set_layout(keys, layout)?;
    let cap = Style {
        fg: wove::Color::Rgb(236, 236, 240),
        bg: wove::Color::Rgb(62, 62, 70),
        ..bold()
    };
    let mut hints = Vec::new();
    for (key, action) in [
        ("←↑↓→", "move"),
        ("enter", "select"),
        ("f", "flip"),
        ("t", "theme"),
        ("p", "pixels"),
        ("n", "new game"),
        (quit, "quit"),
    ] {
        if !hints.is_empty() {
            hints.push(Span::new("  ", Style::default()));
        }
        hints.push(Span::new(format!(" {key} "), cap));
        hints.push(Span::new(format!(" {action}"), dim()));
    }
    let width = hints
        .iter()
        .map(|span| span.text.chars().count())
        .sum::<usize>() as u16;
    tree.add(
        keys,
        RichText {
            spans: hints,
            wrap: Wrap::Word,
            ..Default::default()
        },
    )?;
    Ok(width)
}

/// The name in a five-by-six pixel font, two pixels per row in half blocks,
/// the same way the board draws its pieces.
fn logo() -> String {
    const LETTERS: [[&str; 6]; 5] = [
        [" ####", "#    ", "#    ", "#    ", "#    ", " ####"],
        ["#   #", "#   #", "#####", "#   #", "#   #", "#   #"],
        ["#####", "#    ", "#### ", "#    ", "#    ", "#####"],
        [" ####", "#    ", " ### ", "    #", "    #", "#### "],
        [" ####", "#    ", " ### ", "    #", "    #", "#### "],
    ];
    let mut lines = String::new();
    for row in 0..3 {
        for (index, letter) in LETTERS.iter().enumerate() {
            if index > 0 {
                lines.push(' ');
            }
            let (upper, lower) = (letter[2 * row].as_bytes(), letter[2 * row + 1].as_bytes());
            for col in 0..5 {
                lines.push(match (upper[col] == b'#', lower[col] == b'#') {
                    (true, true) => '█',
                    (true, false) => '▀',
                    (false, true) => '▄',
                    (false, false) => ' ',
                });
            }
        }
        lines.push('\n');
    }
    lines.pop();
    lines
}

fn dot(color: Color) -> &'static str {
    match color {
        Color::White => "●",
        Color::Black => "○",
    }
}

/// A player's card: their name, what they have captured, and their lead.
fn player_card(room: &Room, color: Color) -> Vec<Span> {
    let name = room
        .player(color)
        .map_or("empty seat", |player| player.name.as_str());
    let mut spans = vec![
        Span::new(format!("{} ", dot(color)), Style::default()),
        Span::new(name.to_owned(), bold()),
    ];
    let taken = captured(&room.game, color);
    if !taken.is_empty() {
        spans.push(Span::new(format!("   {taken}"), dim()));
    }
    let lead = material(&room.game, color) - material(&room.game, color.other());
    if lead > 0 {
        spans.push(Span::new(format!(" +{lead}"), dim()));
    }
    spans
}

/// The pieces `color` has taken, as glyphs, in the order the game records.
fn captured(game: &Game, color: Color) -> String {
    let mine = |index: usize| index.is_multiple_of(2) == (color == Color::White);
    let mut order: Vec<Kind> = game
        .moves
        .iter()
        .enumerate()
        .filter(|(index, _)| mine(*index))
        .filter_map(|(_, played)| played.taken)
        .collect();
    order.sort_by_key(|kind| worth(*kind));
    order.iter().map(|kind| glyph(*kind)).collect()
}

/// The pawn-value of the pieces `color` has on the board.
fn material(game: &Game, color: Color) -> i32 {
    Square::all()
        .filter_map(|square| game.position.piece(square))
        .filter(|piece| piece.color == color)
        .map(|piece| worth(piece.kind))
        .sum()
}

fn worth(kind: Kind) -> i32 {
    match kind {
        Kind::Pawn => 1,
        Kind::Knight | Kind::Bishop => 3,
        Kind::Rook => 5,
        Kind::Queen => 9,
        Kind::King => 0,
    }
}

fn glyph(kind: Kind) -> &'static str {
    match kind {
        Kind::Pawn => "♟",
        Kind::Knight => "♞",
        Kind::Bishop => "♝",
        Kind::Rook => "♜",
        Kind::Queen => "♛",
        Kind::King => "♚",
    }
}

/// The score sheet, a move pair per row, with the last move marked.
fn sheet(game: &Game) -> Vec<Span> {
    if game.moves.is_empty() {
        return vec![Span::new("No moves yet", dim())];
    }
    let last = game.moves.len() - 1;
    let mut spans = Vec::new();
    for (index, played) in game.moves.iter().enumerate() {
        if index % 2 == 0 {
            if index > 0 {
                spans.push(Span::new("\n", Style::default()));
            }
            spans.push(Span::new(format!("{:>3}. ", index / 2 + 1), dim()));
        }
        let style = if index == last {
            Style {
                fg: ACCENT,
                ..bold()
            }
        } else {
            Style::default()
        };
        spans.push(Span::new(format!("{:<8}", played.san), style));
    }
    spans
}
