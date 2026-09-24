//! The board element: it paints a position and turns keys and clicks into the
//! move a player chose. The application plays that move; the board never
//! changes a game itself.
//!
//! Squares grow with the space the board is given. Once they are four rows
//! tall the board is drawn as pixels: pieces become shaded pixel art that
//! glides to its square, captures flash, a king in check glows, and legal
//! moves show as dots and rings. A cell holds two pixels with half blocks,
//! or six with sextants where the terminal draws them, which is where the
//! pieces get their detail. Smaller squares fall back to glyphs.
use crate::pieces::{self, Tone};
use crate::rules::{Color, Kind, Move, Piece, Position, Square};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Duration;
use wove::{
    elements::Blocks, render::fit, Button, Canvas, Element, Event, Key, MouseKind, Response, Style,
};

/// The smallest square, in rows; a square is twice as many columns wide.
const MIN: u16 = 2;
/// How long a piece takes to reach its square, and how long a capture flashes.
const GLIDE: Duration = Duration::from_millis(220);
const FLASH: Duration = Duration::from_millis(260);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Rgb(u8, u8, u8);

impl Rgb {
    /// Blend toward `other` by `amount`, from none at 0 to all at 1.
    fn mix(self, other: Rgb, amount: f32) -> Rgb {
        let amount = amount.clamp(0.0, 1.0);
        let channel = |a: u8, b: u8| (f32::from(a) + (f32::from(b) - f32::from(a)) * amount) as u8;
        Rgb(
            channel(self.0, other.0),
            channel(self.1, other.1),
            channel(self.2, other.2),
        )
    }
    fn color(self) -> wove::Color {
        wove::Color::Rgb(self.0, self.1, self.2)
    }
}

/// A board's two square colors; everything else is blended from them.
pub struct Theme {
    light: Rgb,
    dark: Rgb,
}

pub const THEMES: [Theme; 4] = [
    Theme {
        light: Rgb(240, 217, 181),
        dark: Rgb(181, 136, 99),
    },
    Theme {
        light: Rgb(235, 236, 208),
        dark: Rgb(119, 149, 86),
    },
    Theme {
        light: Rgb(222, 227, 230),
        dark: Rgb(140, 162, 173),
    },
    Theme {
        light: Rgb(236, 226, 240),
        dark: Rgb(150, 118, 170),
    },
];

const YELLOW: Rgb = Rgb(255, 214, 92);
const GREEN: Rgb = Rgb(70, 160, 60);
const RED: Rgb = Rgb(230, 60, 40);
const BLACK: Rgb = Rgb(0, 0, 0);
const WHITE: Rgb = Rgb(255, 255, 255);

/// The four tones of a side's pieces: outline, fill, highlight, and shade.
/// Both sides are outlined dark, as in a printed set; the fills tell them
/// apart, and a dark line reads on a square of either color.
const WHITE_TONES: [Rgb; 4] = [
    Rgb(32, 30, 28),
    Rgb(236, 234, 228),
    Rgb(255, 255, 255),
    Rgb(186, 182, 172),
];
const BLACK_TONES: [Rgb; 4] = [
    Rgb(10, 10, 12),
    Rgb(58, 56, 62),
    Rgb(112, 110, 120),
    Rgb(34, 32, 38),
];

/// Rasterized pieces by kind and pixel size.
type Drawings = HashMap<(Kind, usize, usize), Rc<Vec<Tone>>>;

/// A piece on its way to a square.
struct Motion {
    piece: Piece,
    from: Square,
    to: Square,
    start: Duration,
}

pub struct Board {
    position: Position,
    /// The legal moves in `position`, so paint and clicks do not recompute them.
    legal: Vec<Move>,
    last: Option<Move>,
    /// Whether the viewer may move now; otherwise only the cursor roams.
    active: bool,
    /// Rank 1 at the top, as Black sits.
    pub flipped: bool,
    pub cursor: Square,
    pub selected: Option<Square>,
    /// A pawn move waiting for its promotion piece.
    pub promoting: Option<Move>,
    /// The move the viewer chose; the application takes it and plays it.
    pub played: Option<Move>,
    /// An index into `THEMES`.
    pub theme: usize,
    /// How the board is encoded in cells when it is drawn as pixels.
    pub blocks: Blocks,
    /// A card shown over the board, as a headline and a line beneath it.
    pub banner: Option<(String, String)>,
    hover: Option<Square>,
    motions: Vec<Motion>,
    /// A square where a capture just happened, and when.
    flash: Option<(Square, Duration)>,
    /// The application's clock at the last tick.
    now: Duration,
    /// The size the board was last given, which sets the size of a square.
    area: (u16, u16),
    /// Pieces drawn at a pixel size, kept between frames.
    drawn: RefCell<Drawings>,
}

impl Board {
    /// The starting position, seen from `viewer`'s side with the cursor on
    /// their king's pawn.
    pub fn new(viewer: Color) -> Self {
        Self {
            position: Position::start(),
            legal: Vec::new(),
            last: None,
            active: false,
            flipped: viewer == Color::Black,
            cursor: Square::new(4, if viewer == Color::White { 1 } else { 6 }),
            selected: None,
            promoting: None,
            played: None,
            theme: 0,
            blocks: Blocks::Half,
            banner: None,
            hover: None,
            motions: Vec::new(),
            flash: None,
            now: Duration::ZERO,
            area: (0, 0),
            drawn: RefCell::new(HashMap::new()),
        }
    }

    /// Show a position. A new position drops any selection made in the old
    /// one, and its last move, when it is new too, is animated. A position
    /// that no move led to, such as a new game, stops any animation.
    pub fn show(&mut self, position: &Position, last: Option<Move>, active: bool) {
        let changed = self.position != *position;
        if changed {
            let previous = std::mem::replace(&mut self.position, position.clone());
            self.selected = None;
            self.promoting = None;
            match last.filter(|mv| Some(*mv) != self.last) {
                Some(mv) => self.animate(&previous, mv),
                None => {
                    self.motions.clear();
                    self.flash = None;
                }
            }
        }
        // Only a board that may move needs the legal moves.
        if active && (changed || !self.active) {
            self.legal = self.position.legal_moves();
        } else if !active {
            self.legal.clear();
        }
        self.last = last;
        self.active = active;
    }

    /// Start the piece of `mv` gliding from where it was in `previous`, the
    /// rook beside it when the king castled, and a flash where a capture was.
    fn animate(&mut self, previous: &Position, mv: Move) {
        let Some(piece) = self.position.piece(mv.to) else {
            return;
        };
        if let Some((square, _)) = previous.captures(mv) {
            self.flash = Some((square, self.now));
        }
        self.motions.push(Motion {
            piece,
            from: mv.from,
            to: mv.to,
            start: self.now,
        });
        if piece.kind == Kind::King && mv.to.file().abs_diff(mv.from.file()) == 2 {
            let (from, to) = if mv.to.file() == 6 { (7, 5) } else { (0, 3) };
            self.motions.push(Motion {
                piece: Piece {
                    kind: Kind::Rook,
                    ..piece
                },
                from: Square::new(from, mv.from.rank()),
                to: Square::new(to, mv.from.rank()),
                start: self.now,
            });
        }
    }

    /// Advance the clock. Returns whether something is still moving, which
    /// asks the application for another frame soon.
    pub fn tick(&mut self, now: Duration) -> bool {
        self.now = now;
        self.motions.retain(|motion| now < motion.start + GLIDE);
        if self.flash.is_some_and(|(_, at)| now >= at + FLASH) {
            self.flash = None;
        }
        !self.motions.is_empty() || self.flash.is_some()
    }

    /// The height of a square in rows: the largest that fits eight into the
    /// board's area. A square is twice as many columns wide.
    fn cell(&self) -> u16 {
        let (width, height) = self.area;
        (height / 8).min(width / 16).max(MIN)
    }

    /// The square shown at a display column and row.
    fn square(&self, col: u8, row: u8) -> Square {
        if self.flipped {
            Square::new(7 - col, row)
        } else {
            Square::new(col, 7 - row)
        }
    }

    /// The display column and row of a square.
    fn place(&self, square: Square) -> (u8, u8) {
        if self.flipped {
            (7 - square.file(), square.rank())
        } else {
            (square.file(), 7 - square.rank())
        }
    }

    fn square_at(&self, x: u16, y: u16) -> Option<Square> {
        let height = self.cell();
        let col = x / (2 * height);
        let row = y / height;
        (col < 8 && row < 8).then(|| self.square(col as u8, row as u8))
    }

    /// The four squares of the promotion picker, queen first, running from
    /// the promotion square toward the middle of the board.
    fn picker(&self, mv: Move) -> [Square; 4] {
        let (col, row) = self.place(mv.to);
        let rows = if row == 0 { [0, 1, 2, 3] } else { [7, 6, 5, 4] };
        rows.map(|row| self.square(col, row))
    }

    fn step(&mut self, dx: i8, dy: i8) -> Response {
        let (col, row) = self.place(self.cursor);
        let col = (col as i8 + dx).clamp(0, 7) as u8;
        let row = (row as i8 + dy).clamp(0, 7) as u8;
        let next = self.square(col, row);
        if next == self.cursor {
            return Response::HANDLED;
        }
        self.cursor = next;
        Response::REPAINT
    }

    /// Select a piece that can move, or play the selected piece to a square
    /// it can reach. A pawn reaching the last rank waits for its promotion.
    fn choose(&mut self, square: Square) -> Response {
        if !self.active {
            return Response::HANDLED;
        }
        if let Some(from) = self.selected {
            let target = self
                .legal
                .iter()
                .find(|mv| mv.from == from && mv.to == square);
            if let Some(&mv) = target {
                self.selected = None;
                if mv.promotion.is_some() {
                    self.promoting = Some(Move::new(from, square));
                } else {
                    self.played = Some(mv);
                }
                return Response::REPAINT;
            }
        }
        let movable = self.legal.iter().any(|mv| mv.from == square);
        self.selected = (movable && self.selected != Some(square)).then_some(square);
        Response::REPAINT
    }

    fn promote(&mut self, kind: Option<Kind>) -> Response {
        let Some(mv) = self.promoting.take() else {
            return Response::IGNORE;
        };
        if let Some(kind) = kind {
            self.played = Some(Move {
                promotion: Some(kind),
                ..mv
            });
        }
        Response::REPAINT
    }

    fn promotion_key(&mut self, key: Key) -> Response {
        match key {
            Key::Char('q' | 'Q') | Key::Enter => self.promote(Some(Kind::Queen)),
            Key::Char('r' | 'R') => self.promote(Some(Kind::Rook)),
            Key::Char('b' | 'B') => self.promote(Some(Kind::Bishop)),
            Key::Char('n' | 'N') => self.promote(Some(Kind::Knight)),
            Key::Escape => self.promote(None),
            _ => Response::HANDLED,
        }
    }

    fn click(&mut self, square: Square) -> Response {
        if let Some(mv) = self.promoting {
            let choice = self
                .picker(mv)
                .iter()
                .position(|&at| at == square)
                .map(|index| PROMOTIONS[index]);
            return self.promote(choice);
        }
        self.cursor = square;
        self.choose(square)
    }

    fn theme(&self) -> &'static Theme {
        &THEMES[self.theme % THEMES.len()]
    }

    /// The color of a square before anything is drawn on it.
    fn ground(&self, square: Square, focused: bool) -> Rgb {
        let theme = self.theme();
        let dark = (square.file() + square.rank()).is_multiple_of(2);
        let mut color = if dark { theme.dark } else { theme.light };
        if self
            .last
            .is_some_and(|mv| mv.from == square || mv.to == square)
        {
            color = color.mix(Rgb(205, 210, 60), 0.45);
        }
        if Some(square) == self.selected {
            color = color.mix(GREEN, 0.55);
        }
        if focused && self.hover == Some(square) && self.hover != Some(self.cursor) {
            color = color.mix(WHITE, 0.15);
        }
        if let Some((at, since)) = self.flash {
            if at == square {
                let left = 1.0 - (self.now - since).as_secs_f32() / FLASH.as_secs_f32();
                color = color.mix(WHITE, 0.8 * left.clamp(0.0, 1.0));
            }
        }
        color
    }

    fn targets(&self) -> Vec<Square> {
        self.selected
            .map(|from| {
                self.legal
                    .iter()
                    .filter(|mv| mv.from == from)
                    .map(|mv| mv.to)
                    .collect()
            })
            .unwrap_or_default()
    }

    fn checked(&self) -> Option<Square> {
        self.position
            .in_check()
            .then(|| self.position.king(self.position.turn))
            .flatten()
    }

    /// Where a moving piece is between its squares, in display columns and
    /// rows with fractions, easing out as it arrives.
    fn progress(&self, motion: &Motion) -> (f32, f32) {
        let elapsed = self.now.saturating_sub(motion.start).as_secs_f32();
        let t = (elapsed / GLIDE.as_secs_f32()).clamp(0.0, 1.0);
        let t = 1.0 - (1.0 - t) * (1.0 - t);
        let (from_col, from_row) = self.place(motion.from);
        let (to_col, to_row) = self.place(motion.to);
        let along = |a: u8, b: u8| f32::from(a) + (f32::from(b) - f32::from(a)) * t;
        (along(from_col, to_col), along(from_row, to_row))
    }

    /// A piece drawn at a square's size, from the cache or freshly rasterized.
    fn drawing(&self, kind: Kind, (width, height): (usize, usize)) -> Rc<Vec<Tone>> {
        self.drawn
            .borrow_mut()
            .entry((kind, width, height))
            .or_insert_with(|| Rc::new(pieces::raster(kind, width, height)))
            .clone()
    }

    /// Squares whose piece is drawn elsewhere while it moves.
    fn in_motion(&self, square: Square) -> bool {
        self.motions.iter().any(|motion| motion.to == square)
    }

    /// Glyph pieces on colored cells, for squares under four rows.
    fn paint_cells(&self, canvas: &mut Canvas<'_>) {
        let height = self.cell();
        let width = 2 * height;
        let blank = " ".repeat(usize::from(width));
        let middle = (i32::from(width / 2 - 1), i32::from((height - 1) / 2));
        let targets = self.targets();
        let checked = self.checked();
        for row in 0..8u8 {
            for col in 0..8u8 {
                let square = self.square(col, row);
                let x = i32::from(u16::from(col) * width);
                let y = i32::from(u16::from(row) * height);
                let mut ground = self.ground(square, canvas.focused());
                if Some(square) == checked {
                    ground = ground.mix(RED, 0.6);
                }
                if canvas.focused() && square == self.cursor {
                    ground = YELLOW;
                }
                let fill = Style {
                    bg: ground.color(),
                    ..Style::default()
                };
                for line in 0..i32::from(height) {
                    canvas.text(x, y + line, &blank, fill);
                }
                let piece = self
                    .position
                    .piece(square)
                    .filter(|_| !self.in_motion(square));
                if let Some(piece) = piece {
                    let fg = tones_for(piece.color)[1].color();
                    let style = Style { fg, ..fill };
                    canvas.text(x + middle.0, y + middle.1, glyph(piece.kind), style);
                    if targets.contains(&square) {
                        let corner = (x + i32::from(width) - 1, y + i32::from(height) - 1);
                        canvas.text(corner.0, corner.1, "•", fill);
                    }
                } else if targets.contains(&square) {
                    canvas.text(x + middle.0, y + middle.1, "•", fill);
                }
                self.coordinate(canvas, col, row, (x, y), (width, height), ground);
            }
        }
        for motion in &self.motions {
            let (col, row) = self.progress(motion);
            let x = (col * f32::from(width)).round() as i32 + middle.0;
            let y = (row * f32::from(height)).round() as i32 + middle.1;
            let style = Style {
                fg: tones_for(motion.piece.color)[1].color(),
                ..Style::default()
            };
            canvas.text(x, y, glyph(motion.piece.kind), style);
        }
        if let Some(mv) = self.promoting {
            self.glyph_picker(canvas, mv, (width, height), middle);
        }
    }

    /// The promotion choices as glyphs on pale squares.
    fn glyph_picker(
        &self,
        canvas: &mut Canvas<'_>,
        mv: Move,
        (width, height): (u16, u16),
        middle: (i32, i32),
    ) {
        let blank = " ".repeat(usize::from(width));
        let mover = self.position.turn;
        let fill = Style {
            bg: Rgb(250, 250, 245).color(),
            ..Style::default()
        };
        let ink = Style {
            fg: tones_for(mover)[if mover == Color::White { 3 } else { 1 }].color(),
            ..fill
        };
        for (index, square) in self.picker(mv).into_iter().enumerate() {
            let (col, row) = self.place(square);
            let x = i32::from(u16::from(col) * width);
            let y = i32::from(u16::from(row) * height);
            for line in 0..i32::from(height) {
                canvas.text(x, y + line, &blank, fill);
            }
            canvas.text(x + middle.0, y + middle.1, glyph(PROMOTIONS[index]), ink);
        }
    }

    /// The rank number in the first column of squares and the file letter in
    /// the last row, each in the opposite shade of its square.
    fn coordinate(
        &self,
        canvas: &mut Canvas<'_>,
        col: u8,
        row: u8,
        (x, y): (i32, i32),
        (width, height): (u16, u16),
        ground: Rgb,
    ) {
        let square = self.square(col, row);
        let theme = self.theme();
        let dark = (square.file() + square.rank()).is_multiple_of(2);
        let ink = if dark { theme.light } else { theme.dark };
        let style = Style {
            fg: ink.color(),
            bg: ground.color(),
            ..Style::default()
        };
        if col == 0 {
            canvas.text(x, y, &(square.rank() + 1).to_string(), style);
        }
        if row == 7 {
            let file = ((b'a' + square.file()) as char).to_string();
            canvas.text(
                x + i32::from(width) - 1,
                y + i32::from(height) - 1,
                &file,
                style,
            );
        }
    }

    /// The pixel board, for squares four rows and taller: the squares and
    /// their marks, then the pieces, then whatever floats above them.
    fn paint_pixels(&self, canvas: &mut Canvas<'_>) {
        let height = self.cell();
        let (across, down) = self.blocks.per_cell();
        // A square's pixels: as many cells as it is wide and tall, times
        // the pixels a cell holds each way.
        let square = (usize::from(2 * height) * across, usize::from(height) * down);
        let mut pixels = Grid::new((8 * square.0, 8 * square.1));
        let focused = canvas.focused();
        self.pixel_squares(&mut pixels, square, focused);
        self.pixel_pieces(&mut pixels, square);
        if let Some(mv) = self.promoting {
            self.pixel_picker(&mut pixels, square, mv);
        }
        pixels.flush(canvas, self.blocks);
        self.pixel_coordinates(canvas, height, focused);
    }

    /// The top-left pixel of a display square.
    fn origin(col: u8, row: u8, square: (usize, usize)) -> (usize, usize) {
        (usize::from(col) * square.0, usize::from(row) * square.1)
    }

    /// Every square's ground, with the glow of a check, the dots and rings
    /// of legal moves, and the cursor's frame.
    fn pixel_squares(&self, pixels: &mut Grid, square: (usize, usize), focused: bool) {
        let targets = self.targets();
        let checked = self.checked();
        for row in 0..8u8 {
            for col in 0..8u8 {
                let here = self.square(col, row);
                let origin = Self::origin(col, row, square);
                let ground = self.ground(here, focused);
                pixels.rect(origin, square, ground);
                if Some(here) == checked {
                    pixels.glow(origin, square, RED);
                }
                if targets.contains(&here) {
                    if self.position.piece(here).is_some() {
                        pixels.ring(origin, square, ground.mix(BLACK, 0.35));
                    } else {
                        pixels.disc(origin, square, ground.mix(BLACK, 0.3));
                    }
                }
                if focused && here == self.cursor {
                    pixels.ring(origin, square, YELLOW);
                }
            }
        }
    }

    /// The pieces at rest on their squares, then those still gliding.
    fn pixel_pieces(&self, pixels: &mut Grid, square: (usize, usize)) {
        for row in 0..8u8 {
            for col in 0..8u8 {
                let here = self.square(col, row);
                if let Some(piece) = self.position.piece(here).filter(|_| !self.in_motion(here)) {
                    let drawing = self.drawing(piece.kind, square);
                    pixels.sprite(
                        Self::origin(col, row, square),
                        square.0,
                        &drawing,
                        piece.color,
                    );
                }
            }
        }
        for motion in &self.motions {
            let (col, row) = self.progress(motion);
            let origin = (
                (col * square.0 as f32).round() as usize,
                (row * square.1 as f32).round() as usize,
            );
            let drawing = self.drawing(motion.piece.kind, square);
            pixels.sprite(origin, square.0, &drawing, motion.piece.color);
        }
    }

    /// The promotion choices on pale squares over a dimmed board.
    fn pixel_picker(&self, pixels: &mut Grid, square: (usize, usize), mv: Move) {
        pixels.dim(0.45);
        let mover = self.position.turn;
        for (index, here) in self.picker(mv).into_iter().enumerate() {
            let (col, row) = self.place(here);
            let origin = Self::origin(col, row, square);
            pixels.rect(origin, square, Rgb(250, 250, 245));
            pixels.ring(origin, square, Rgb(60, 60, 60));
            let drawing = self.drawing(PROMOTIONS[index], square);
            pixels.sprite(origin, square.0, &drawing, mover);
        }
    }

    /// Ranks down the first column and files along the last row, written
    /// over the flushed cells.
    fn pixel_coordinates(&self, canvas: &mut Canvas<'_>, height: u16, focused: bool) {
        let size = (2 * height, height);
        let edge: Vec<(u8, u8)> = (0..8u8)
            .flat_map(|row| [(0, row), (7, row)])
            .chain((1..7u8).map(|col| (col, 7)))
            .collect();
        for (col, row) in edge {
            let here = self.square(col, row);
            let x = i32::from(u16::from(col) * size.0);
            let y = i32::from(u16::from(row) * height);
            let ground = self.ground(here, focused);
            self.coordinate(canvas, col, row, (x, y), size, ground);
        }
    }
}

/// The promotion choices in picker order.
const PROMOTIONS: [Kind; 4] = [Kind::Queen, Kind::Rook, Kind::Bishop, Kind::Knight];

fn tones_for(color: Color) -> &'static [Rgb; 4] {
    match color {
        Color::White => &WHITE_TONES,
        Color::Black => &BLACK_TONES,
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

impl Element for Board {
    fn measure(&self, _width: Option<u16>) -> (u16, u16) {
        (16 * MIN, 8 * MIN)
    }

    fn focusable(&self) -> bool {
        true
    }

    fn event(&mut self, event: &Event) -> Response {
        match event {
            Event::Key(key, _) if self.promoting.is_some() => self.promotion_key(*key),
            Event::Key(key, _) => match key {
                Key::Left | Key::Char('h') => self.step(-1, 0),
                Key::Right | Key::Char('l') => self.step(1, 0),
                Key::Up | Key::Char('k') => self.step(0, -1),
                Key::Down | Key::Char('j') => self.step(0, 1),
                Key::Enter | Key::Char(' ') => self.choose(self.cursor),
                Key::Escape if self.selected.is_some() => {
                    self.selected = None;
                    Response::REPAINT
                }
                _ => Response::IGNORE,
            },
            Event::Mouse(mouse) => match mouse.kind {
                MouseKind::Down(Button::Left) => match self.square_at(mouse.x, mouse.y) {
                    Some(square) => self.click(square),
                    None => Response::IGNORE,
                },
                MouseKind::Move => {
                    let over = self.square_at(mouse.x, mouse.y);
                    if over == self.hover {
                        Response::HANDLED
                    } else {
                        self.hover = over;
                        Response::REPAINT
                    }
                }
                _ => Response::IGNORE,
            },
            Event::Leave => {
                self.hover = None;
                Response::REPAINT
            }
            _ => Response::IGNORE,
        }
    }

    fn viewport(&mut self, size: (u16, u16), _content: (u32, u32)) -> (u32, u32) {
        self.area = size;
        (0, 0)
    }

    fn paint(&self, canvas: &mut Canvas<'_>) {
        if self.cell() >= 4 {
            self.paint_pixels(canvas);
        } else {
            self.paint_cells(canvas);
        }
        if let Some((headline, detail)) = &self.banner {
            self.overlay(canvas, headline, detail);
        }
    }
}

impl Board {
    /// A rounded card in the middle of the board, no wider than the board.
    fn overlay(&self, canvas: &mut Canvas<'_>, headline: &str, detail: &str) {
        let (cols, rows) = (i32::from(16 * self.cell()), i32::from(8 * self.cell()));
        let width = (headline.chars().count().max(detail.chars().count()) as i32 + 6).min(cols);
        let inner = (width - 4).max(0) as usize;
        let (headline, detail) = (fit(headline, inner), fit(detail, inner));
        let height = 5;
        let (x, y) = ((cols - width) / 2, (rows - height) / 2);
        let paper = Style {
            fg: Rgb(232, 230, 224).color(),
            bg: Rgb(28, 28, 32).color(),
            ..Style::default()
        };
        let blank = " ".repeat(width.max(2) as usize - 2);
        canvas.text(x, y, &format!("╭{}╮", "─".repeat(blank.len())), paper);
        for line in 1..height - 1 {
            canvas.text(x, y + line, &format!("│{blank}│"), paper);
        }
        canvas.text(
            x,
            y + height - 1,
            &format!("╰{}╯", "─".repeat(blank.len())),
            paper,
        );
        let centered = |text: &str| x + (width - text.chars().count() as i32) / 2;
        canvas.text(
            centered(headline),
            y + 1,
            headline,
            Style {
                fg: YELLOW.color(),
                bold: true,
                ..paper
            },
        );
        canvas.text(centered(detail), y + 3, detail, paper);
    }
}

/// A grid of pixels that flushes to cells: with half blocks a cell shows
/// the pixel above in its glyph and the one below in its background; with
/// sextants each cell's six pixels are reduced to its two nearest colors and
/// the glyph whose pattern tells them apart.
struct Grid {
    width: usize,
    height: usize,
    data: Vec<Rgb>,
    /// How much each pixel's color matters when a cell is reduced to two:
    /// a piece's outline is what makes it legible, so it weighs more.
    weight: Vec<u8>,
}

impl Grid {
    fn new((width, height): (usize, usize)) -> Self {
        Self {
            width,
            height,
            data: vec![BLACK; width * height],
            weight: vec![1; width * height],
        }
    }

    fn set(&mut self, x: usize, y: usize, color: Rgb) {
        if x < self.width && y < self.height {
            self.data[y * self.width + x] = color;
            self.weight[y * self.width + x] = 1;
        }
    }

    fn set_weighted(&mut self, x: usize, y: usize, color: Rgb, weight: u8) {
        self.set(x, y, color);
        if x < self.width && y < self.height {
            self.weight[y * self.width + x] = weight;
        }
    }

    fn get(&self, x: usize, y: usize) -> Rgb {
        self.data[y * self.width + x]
    }

    fn rect(&mut self, (x, y): (usize, usize), (width, height): (usize, usize), color: Rgb) {
        for py in y..y + height {
            for px in x..x + width {
                self.set(px, py, color);
            }
        }
    }

    /// A border just inside a square, an eighth of the square thick.
    fn ring(&mut self, (x, y): (usize, usize), (width, height): (usize, usize), color: Rgb) {
        let (tx, ty) = ((width / 8).max(1), (height / 8).max(1));
        for py in 0..height {
            for px in 0..width {
                if px < tx || py < ty || px >= width - tx || py >= height - ty {
                    self.set(x + px, y + py, color);
                }
            }
        }
    }

    /// How far a pixel is from the middle of a square, as a share of the
    /// way to its edge, so shapes stay round whatever a pixel's proportions.
    fn distance((width, height): (usize, usize), px: usize, py: usize) -> f32 {
        let dx = (px as f32 + 0.5) / width as f32 * 2.0 - 1.0;
        let dy = (py as f32 + 0.5) / height as f32 * 2.0 - 1.0;
        (dx * dx + dy * dy).sqrt()
    }

    /// A filled circle in the middle of a square.
    fn disc(&mut self, (x, y): (usize, usize), square: (usize, usize), color: Rgb) {
        for py in 0..square.1 {
            for px in 0..square.0 {
                if Self::distance(square, px, py) <= 0.34 {
                    self.set(x + px, y + py, color);
                }
            }
        }
    }

    /// A color strongest at the middle of a square, fading to its edges.
    fn glow(&mut self, (x, y): (usize, usize), square: (usize, usize), color: Rgb) {
        for py in 0..square.1 {
            for px in 0..square.0 {
                let amount = (1.0 - Self::distance(square, px, py)).clamp(0.0, 1.0) * 0.9;
                let under = self.get(x + px, y + py);
                self.set(x + px, y + py, under.mix(color, amount));
            }
        }
    }

    /// Darken everything, for a picker to stand out.
    fn dim(&mut self, amount: f32) {
        for pixel in &mut self.data {
            *pixel = pixel.mix(BLACK, amount);
        }
    }

    /// Draw a piece rasterized at `width` pixels across, in a side's tones.
    fn sprite(&mut self, (x, y): (usize, usize), width: usize, tones: &[Tone], color: Color) {
        let palette = tones_for(color);
        for (index, tone) in tones.iter().enumerate() {
            let (color, weight) = match tone {
                Tone::None => continue,
                Tone::Outline => (palette[0], 3),
                Tone::Fill => (palette[1], 1),
                Tone::Highlight => (palette[2], 1),
                Tone::Shade => (palette[3], 1),
            };
            let (px, py) = (index % width, index / width);
            self.set_weighted(x + px, y + py, color, weight);
        }
    }

    fn flush(&self, canvas: &mut Canvas<'_>, mode: Blocks) {
        let (across, down) = mode.per_cell();
        let mut block = Vec::with_capacity(across * down);
        for row in 0..self.height / down {
            for col in 0..self.width / across {
                block.clear();
                for dy in 0..down {
                    for dx in 0..across {
                        let (x, y) = (col * across + dx, row * down + dy);
                        block.push((self.get(x, y), self.weight[y * self.width + x]));
                    }
                }
                let (fg, bg, bits) = split(&block);
                let glyph = mode.glyph(bits);
                let style = Style {
                    fg: fg.color(),
                    bg: bg.color(),
                    ..Style::default()
                };
                canvas.text(
                    col as i32,
                    row as i32,
                    glyph.encode_utf8(&mut [0; 4]),
                    style,
                );
            }
        }
    }
}

/// Reduce a cell's weighted pixels to two colors: of every pair of colors
/// present, the one that loses least when each pixel takes the nearer of
/// the two, a pixel's loss counting as much as its weight. Returns the pair
/// and a bit per pixel that takes the first, in reading order. A cell of
/// one color has no bits set.
fn split(block: &[(Rgb, u8)]) -> (Rgb, Rgb, u8) {
    let mut colors: Vec<Rgb> = Vec::with_capacity(block.len());
    for &(pixel, _) in block {
        if !colors.contains(&pixel) {
            colors.push(pixel);
        }
    }
    let apart = |a: Rgb, b: Rgb| {
        let d = |x: u8, y: u8| (f32::from(x) - f32::from(y)).powi(2);
        d(a.0, b.0) + d(a.1, b.1) + d(a.2, b.2)
    };
    let mut best = (colors[0], colors[0], f32::INFINITY);
    for (i, &fg) in colors.iter().enumerate() {
        for &bg in &colors[i + 1..] {
            let loss: f32 = block
                .iter()
                .map(|&(pixel, weight)| f32::from(weight) * apart(pixel, fg).min(apart(pixel, bg)))
                .sum();
            if loss < best.2 {
                best = (fg, bg, loss);
            }
        }
    }
    let (fg, bg, _) = best;
    let mut bits = 0;
    for (index, &(pixel, _)) in block.iter().enumerate() {
        if fg != bg && apart(pixel, fg) < apart(pixel, bg) {
            bits |= 1 << index;
        }
    }
    (fg, bg, bits)
}
