//! The rules: pieces, positions, legal moves, standard algebraic notation,
//! and the results a game can reach. Nothing here knows about terminals; the
//! board element and the sessions build on it.
use Kind::{Bishop, King, Knight, Pawn, Queen, Rook};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Color {
    White,
    Black,
}

impl Color {
    pub fn other(self) -> Self {
        match self {
            Self::White => Self::Black,
            Self::Black => Self::White,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::White => "White",
            Self::Black => "Black",
        }
    }
    /// The direction this color's pawns advance, as a rank step.
    fn forward(self) -> i8 {
        match self {
            Self::White => 1,
            Self::Black => -1,
        }
    }
    /// The rank this color's king and rooks start on.
    fn home(self) -> u8 {
        match self {
            Self::White => 0,
            Self::Black => 7,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    Pawn,
    Knight,
    Bishop,
    Rook,
    Queen,
    King,
}

impl Kind {
    /// The letter notation uses; a pawn has none.
    fn letter(self) -> &'static str {
        match self {
            Pawn => "",
            Knight => "N",
            Bishop => "B",
            Rook => "R",
            Queen => "Q",
            King => "K",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Piece {
    pub color: Color,
    pub kind: Kind,
}

/// A square indexed a1 = 0 through h8 = 63, file first.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Square(u8);

impl Square {
    pub fn new(file: u8, rank: u8) -> Self {
        Self(rank * 8 + file)
    }
    pub fn file(self) -> u8 {
        self.0 % 8
    }
    pub fn rank(self) -> u8 {
        self.0 / 8
    }
    /// The square's name, such as `e4`.
    pub fn name(self) -> String {
        format!("{}{}", (b'a' + self.file()) as char, self.rank() + 1)
    }
    /// The square a step away, or `None` off the board.
    pub fn offset(self, file: i8, rank: i8) -> Option<Self> {
        let file = i8::try_from(self.file()).ok()? + file;
        let rank = i8::try_from(self.rank()).ok()? + rank;
        ((0..8).contains(&file) && (0..8).contains(&rank))
            .then(|| Self::new(file as u8, rank as u8))
    }
    pub fn all() -> impl Iterator<Item = Self> {
        (0..64).map(Self)
    }
    fn index(self) -> usize {
        usize::from(self.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Move {
    pub from: Square,
    pub to: Square,
    /// The piece a pawn becomes on the last rank.
    pub promotion: Option<Kind>,
}

impl Move {
    pub fn new(from: Square, to: Square) -> Self {
        Self {
            from,
            to,
            promotion: None,
        }
    }
}

const KNIGHT_STEPS: [(i8, i8); 8] = [
    (1, 2),
    (2, 1),
    (2, -1),
    (1, -2),
    (-1, -2),
    (-2, -1),
    (-2, 1),
    (-1, 2),
];
const KING_STEPS: [(i8, i8); 8] = [
    (0, 1),
    (1, 1),
    (1, 0),
    (1, -1),
    (0, -1),
    (-1, -1),
    (-1, 0),
    (-1, 1),
];
const DIAGONALS: [(i8, i8); 4] = [(1, 1), (1, -1), (-1, -1), (-1, 1)];
const LINES: [(i8, i8); 4] = [(0, 1), (1, 0), (0, -1), (-1, 0)];

/// The board and everything else the rules need to know about a moment in a game.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Position {
    squares: [Option<Piece>; 64],
    pub turn: Color,
    /// White king side, white queen side, black king side, black queen side.
    castling: [bool; 4],
    /// The square a pawn passed with its double step, capturable this move.
    en_passant: Option<Square>,
    /// Moves since the last capture or pawn move, for the fifty-move rule.
    halfmove: u16,
}

impl Position {
    pub fn start() -> Self {
        let mut squares = [None; 64];
        let back = [Rook, Knight, Bishop, Queen, King, Bishop, Knight, Rook];
        for (file, &kind) in back.iter().enumerate() {
            let file = file as u8;
            let mut place = |rank, color, kind| {
                squares[Square::new(file, rank).index()] = Some(Piece { color, kind });
            };
            place(0, Color::White, kind);
            place(1, Color::White, Pawn);
            place(6, Color::Black, Pawn);
            place(7, Color::Black, kind);
        }
        Self {
            squares,
            turn: Color::White,
            castling: [true; 4],
            en_passant: None,
            halfmove: 0,
        }
    }

    pub fn piece(&self, square: Square) -> Option<Piece> {
        self.squares[square.index()]
    }

    pub fn king(&self, color: Color) -> Option<Square> {
        Square::all().find(|&square| self.piece(square) == Some(Piece { color, kind: King }))
    }

    /// Whether `by` could capture on `square` if it were their move.
    fn attacked(&self, square: Square, by: Color) -> bool {
        let has = |step: (i8, i8), kinds: &[Kind]| {
            square
                .offset(step.0, step.1)
                .and_then(|at| self.piece(at))
                .is_some_and(|piece| piece.color == by && kinds.contains(&piece.kind))
        };
        // Pawns attack the rank ahead of them, so one attacking `square` sits behind it.
        let behind = -by.forward();
        if has((-1, behind), &[Pawn]) || has((1, behind), &[Pawn]) {
            return true;
        }
        if KNIGHT_STEPS.iter().any(|&step| has(step, &[Knight]))
            || KING_STEPS.iter().any(|&step| has(step, &[King]))
        {
            return true;
        }
        let rays = [(DIAGONALS, [Bishop, Queen]), (LINES, [Rook, Queen])];
        rays.iter().any(|(steps, kinds)| {
            steps.iter().any(|&(file, rank)| {
                let mut at = square.offset(file, rank);
                while let Some(here) = at {
                    match self.piece(here) {
                        Some(piece) => return piece.color == by && kinds.contains(&piece.kind),
                        None => at = here.offset(file, rank),
                    }
                }
                false
            })
        })
    }

    pub fn in_check(&self) -> bool {
        self.king(self.turn)
            .is_some_and(|king| self.attacked(king, self.turn.other()))
    }

    /// Every legal move for the side to move.
    pub fn legal_moves(&self) -> Vec<Move> {
        let mut moves = Vec::new();
        for from in Square::all() {
            let Some(piece) = self.piece(from) else {
                continue;
            };
            if piece.color != self.turn {
                continue;
            }
            match piece.kind {
                Pawn => self.pawn_moves(from, &mut moves),
                Knight => self.step_moves(from, &KNIGHT_STEPS, &mut moves),
                Bishop => self.ray_moves(from, &DIAGONALS, &mut moves),
                Rook => self.ray_moves(from, &LINES, &mut moves),
                Queen => {
                    self.ray_moves(from, &DIAGONALS, &mut moves);
                    self.ray_moves(from, &LINES, &mut moves);
                }
                King => {
                    self.step_moves(from, &KING_STEPS, &mut moves);
                    self.castle_moves(from, &mut moves);
                }
            }
        }
        // A move that leaves the mover's king attacked is not a move.
        moves.retain(|&mv| {
            let mut next = self.clone();
            next.apply(mv);
            next.king(self.turn)
                .is_none_or(|king| !next.attacked(king, self.turn.other()))
        });
        moves
    }

    pub fn is_legal(&self, mv: Move) -> bool {
        self.legal_moves().contains(&mv)
    }

    /// What a move takes, and from which square: the piece on the target,
    /// or the pawn beside a pawn that captures en passant.
    pub fn captures(&self, mv: Move) -> Option<(Square, Kind)> {
        if let Some(piece) = self.piece(mv.to) {
            return Some((mv.to, piece.kind));
        }
        let pawn = self.piece(mv.from).is_some_and(|piece| piece.kind == Pawn);
        (pawn && self.en_passant == Some(mv.to))
            .then(|| (Square::new(mv.to.file(), mv.from.rank()), Pawn))
    }

    /// The position after a move the generator produced.
    pub fn after(&self, mv: Move) -> Self {
        let mut next = self.clone();
        next.apply(mv);
        next
    }

    /// Moves since the last capture or pawn move.
    pub fn halfmove(&self) -> u16 {
        self.halfmove
    }

    fn step_moves(&self, from: Square, steps: &[(i8, i8)], moves: &mut Vec<Move>) {
        for &(file, rank) in steps {
            if let Some(to) = from.offset(file, rank) {
                if self.piece(to).is_none_or(|piece| piece.color != self.turn) {
                    moves.push(Move::new(from, to));
                }
            }
        }
    }

    fn ray_moves(&self, from: Square, steps: &[(i8, i8)], moves: &mut Vec<Move>) {
        for &(file, rank) in steps {
            let mut at = from.offset(file, rank);
            while let Some(to) = at {
                match self.piece(to) {
                    Some(piece) => {
                        if piece.color != self.turn {
                            moves.push(Move::new(from, to));
                        }
                        break;
                    }
                    None => {
                        moves.push(Move::new(from, to));
                        at = to.offset(file, rank);
                    }
                }
            }
        }
    }

    fn pawn_moves(&self, from: Square, moves: &mut Vec<Move>) {
        let forward = self.turn.forward();
        let last = self.turn.other().home();
        let start = if self.turn == Color::White { 1 } else { 6 };
        let mut push = |to: Square| {
            if to.rank() == last {
                for kind in [Queen, Rook, Bishop, Knight] {
                    moves.push(Move {
                        from,
                        to,
                        promotion: Some(kind),
                    });
                }
            } else {
                moves.push(Move::new(from, to));
            }
        };
        if let Some(to) = from.offset(0, forward) {
            if self.piece(to).is_none() {
                push(to);
                if let Some(two) = to.offset(0, forward) {
                    if from.rank() == start && self.piece(two).is_none() {
                        push(two);
                    }
                }
            }
        }
        for file in [-1, 1] {
            if let Some(to) = from.offset(file, forward) {
                let enemy = self.piece(to).is_some_and(|piece| piece.color != self.turn);
                if enemy || self.en_passant == Some(to) {
                    push(to);
                }
            }
        }
    }

    /// Castling: the king on its home square, not in check, with the right kept,
    /// the rook in place, the squares between empty, and the king's path safe.
    fn castle_moves(&self, from: Square, moves: &mut Vec<Move>) {
        let home = self.turn.home();
        if from != Square::new(4, home) || self.in_check() {
            return;
        }
        let rights = if self.turn == Color::White {
            [self.castling[0], self.castling[1]]
        } else {
            [self.castling[2], self.castling[3]]
        };
        let enemy = self.turn.other();
        let at = |file| Square::new(file, home);
        let empty = |files: &[u8]| files.iter().all(|&file| self.piece(at(file)).is_none());
        let safe = |files: &[u8]| files.iter().all(|&file| !self.attacked(at(file), enemy));
        let rook = |file| {
            self.piece(at(file))
                == Some(Piece {
                    color: self.turn,
                    kind: Rook,
                })
        };
        if rights[0] && rook(7) && empty(&[5, 6]) && safe(&[5, 6]) {
            moves.push(Move::new(from, at(6)));
        }
        if rights[1] && rook(0) && empty(&[1, 2, 3]) && safe(&[2, 3]) {
            moves.push(Move::new(from, at(2)));
        }
    }

    /// Make a move the generator produced, without checking it again.
    fn apply(&mut self, mv: Move) {
        let Some(piece) = self.piece(mv.from) else {
            return;
        };
        let capture = self.piece(mv.to).is_some();
        self.halfmove = if capture || piece.kind == Pawn {
            0
        } else {
            self.halfmove + 1
        };
        // En passant takes the pawn that passed, which is not on the target square.
        if piece.kind == Pawn && self.en_passant == Some(mv.to) {
            self.squares[Square::new(mv.to.file(), mv.from.rank()).index()] = None;
        }
        // A double step is recorded only when an enemy pawn stands ready to
        // take it, so positions that differ in nothing a player can use
        // compare equal for the repetition rule.
        self.en_passant = None;
        if piece.kind == Pawn && mv.to.rank().abs_diff(mv.from.rank()) == 2 {
            let enemy = Piece {
                color: piece.color.other(),
                kind: Pawn,
            };
            let beside = [-1, 1]
                .into_iter()
                .filter_map(|file| mv.to.offset(file, 0))
                .any(|square| self.piece(square) == Some(enemy));
            if beside {
                self.en_passant = Some(Square::new(
                    mv.from.file(),
                    (mv.from.rank() + mv.to.rank()) / 2,
                ));
            }
        }
        // Castling is a king move of two files; the rook lands beside the king.
        if piece.kind == King && mv.to.file().abs_diff(mv.from.file()) == 2 {
            let (rook_from, rook_to) = if mv.to.file() == 6 { (7, 5) } else { (0, 3) };
            let rank = mv.from.rank();
            let rook = self.squares[Square::new(rook_from, rank).index()].take();
            self.squares[Square::new(rook_to, rank).index()] = rook;
        }
        // Leaving or landing on a king or rook home square ends the rights it carried.
        for square in [mv.from, mv.to] {
            match (square.file(), square.rank()) {
                (4, 0) => self.castling[0..2].fill(false),
                (7, 0) => self.castling[0] = false,
                (0, 0) => self.castling[1] = false,
                (4, 7) => self.castling[2..4].fill(false),
                (7, 7) => self.castling[2] = false,
                (0, 7) => self.castling[3] = false,
                _ => {}
            }
        }
        self.squares[mv.from.index()] = None;
        self.squares[mv.to.index()] = Some(Piece {
            kind: mv.promotion.unwrap_or(piece.kind),
            ..piece
        });
        self.turn = self.turn.other();
    }

    /// The move in standard algebraic notation, as read from this position.
    pub fn san(&self, mv: Move) -> String {
        let Some(piece) = self.piece(mv.from) else {
            return String::new();
        };
        let mut text = String::new();
        if piece.kind == King && mv.to.file().abs_diff(mv.from.file()) == 2 {
            text.push_str(if mv.to.file() == 6 { "O-O" } else { "O-O-O" });
        } else {
            let capture = self.piece(mv.to).is_some()
                || (piece.kind == Pawn && mv.to.file() != mv.from.file());
            if piece.kind == Pawn {
                if capture {
                    text.push((b'a' + mv.from.file()) as char);
                }
            } else {
                text.push_str(piece.kind.letter());
                // Another piece of the kind that reaches the same square needs
                // the origin spelled out: its file, its rank, or both.
                let others: Vec<Square> = self
                    .legal_moves()
                    .into_iter()
                    .filter(|other| {
                        other.to == mv.to
                            && other.from != mv.from
                            && self.piece(other.from) == Some(piece)
                    })
                    .map(|other| other.from)
                    .collect();
                if !others.is_empty() {
                    let name = mv.from.name();
                    if others.iter().all(|other| other.file() != mv.from.file()) {
                        text.push_str(&name[..1]);
                    } else if others.iter().all(|other| other.rank() != mv.from.rank()) {
                        text.push_str(&name[1..]);
                    } else {
                        text.push_str(&name);
                    }
                }
            }
            if capture {
                text.push('x');
            }
            text.push_str(&mv.to.name());
            if let Some(kind) = mv.promotion {
                text.push('=');
                text.push_str(kind.letter());
            }
        }
        let mut next = self.clone();
        next.apply(mv);
        if next.in_check() {
            text.push(if next.legal_moves().is_empty() {
                '#'
            } else {
                '+'
            });
        }
        text
    }

    /// Neither side can force mate: only kings, or a lone minor piece, or
    /// bishops that all stand on squares of one color.
    fn insufficient_material(&self) -> bool {
        let mut minors = Vec::new();
        for square in Square::all() {
            match self.piece(square).map(|piece| piece.kind) {
                Some(Pawn | Rook | Queen) => return false,
                Some(Bishop) => minors.push(Some((square.file() + square.rank()) % 2)),
                Some(Knight) => minors.push(None),
                _ => {}
            }
        }
        match minors.as_slice() {
            [] | [_] => true,
            [Some(first), rest @ ..] => rest.iter().all(|other| *other == Some(*first)),
            _ => false,
        }
    }

    /// Equal for the repetition rule: the same pieces, side to move, and rights.
    fn repeats(&self, other: &Self) -> bool {
        self.squares == other.squares
            && self.turn == other.turn
            && self.castling == other.castling
            && self.en_passant == other.en_passant
    }
}

/// How a game ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Checkmate(Color),
    Stalemate,
    Repetition,
    FiftyMoves,
    Insufficient,
}

impl Outcome {
    /// What happened and why, as a headline and its detail.
    pub fn headline(self) -> (&'static str, String) {
        match self {
            Self::Checkmate(winner) => ("Checkmate", format!("{} wins", winner.name())),
            Self::Stalemate => ("Draw", "stalemate".into()),
            Self::Repetition => ("Draw", "threefold repetition".into()),
            Self::FiftyMoves => ("Draw", "fifty moves without progress".into()),
            Self::Insufficient => ("Draw", "insufficient material".into()),
        }
    }
    pub fn describe(self) -> String {
        let (headline, detail) = self.headline();
        format!("{headline} · {detail}")
    }
}

/// A move that was played: its notation and what it took.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Played {
    pub mv: Move,
    pub san: String,
    pub taken: Option<Kind>,
}

/// A game from the starting position: the moves played and how it stands.
pub struct Game {
    pub position: Position,
    pub moves: Vec<Played>,
    /// Every position reached, for the repetition rule.
    history: Vec<Position>,
    pub outcome: Option<Outcome>,
}

impl Game {
    pub fn new() -> Self {
        Self::from(Position::start())
    }

    fn from(position: Position) -> Self {
        Self {
            history: vec![position.clone()],
            position,
            moves: Vec::new(),
            outcome: None,
        }
    }

    pub fn last_move(&self) -> Option<Move> {
        self.moves.last().map(|played| played.mv)
    }

    /// Play a legal move. A finished game and an illegal move are refused.
    pub fn play(&mut self, mv: Move) -> Result<(), &'static str> {
        if self.outcome.is_some() {
            return Err("the game is over");
        }
        if !self.position.is_legal(mv) {
            return Err("illegal move");
        }
        let san = self.position.san(mv);
        let taken = self.position.captures(mv).map(|(_, kind)| kind);
        self.position.apply(mv);
        self.moves.push(Played { mv, san, taken });
        self.history.push(self.position.clone());
        self.outcome = self.judge();
        Ok(())
    }

    fn judge(&self) -> Option<Outcome> {
        let position = &self.position;
        if position.legal_moves().is_empty() {
            return Some(if position.in_check() {
                Outcome::Checkmate(position.turn.other())
            } else {
                Outcome::Stalemate
            });
        }
        if position.halfmove >= 100 {
            return Some(Outcome::FiftyMoves);
        }
        if position.insufficient_material() {
            return Some(Outcome::Insufficient);
        }
        let seen = self
            .history
            .iter()
            .filter(|past| past.repeats(position))
            .count();
        (seen >= 3).then_some(Outcome::Repetition)
    }
}
