//! A small engine to play against: iterative deepening, alpha-beta over the
//! rules' legal moves, quiescence on captures, and a material and
//! piece-square evaluation. It is a club player, not a grandmaster. It exists
//! so one person can play, and to show a worker thread reaching the screen.
use crate::rules::{Color, Kind, Move, Position, Square};
use std::time::{Duration, Instant};

/// A mate score; closer mates score higher by the plies they take.
pub const MATE: i32 = 100_000;
/// How many capture-only plies the quiescence search follows.
const QUIET_PLIES: u32 = 4;

/// The best move found within `budget`, or `None` with no legal move.
pub fn choose(position: &Position, budget: Duration) -> Option<Move> {
    search(position, budget).map(|(mv, _)| mv)
}

/// The position's value in centipawns from White's side, positive when White
/// stands better. At `MATE` and beyond a mate is forced; the plies it takes
/// are the difference.
pub fn analyse(position: &Position, budget: Duration) -> i32 {
    let score = search(position, budget).map_or_else(
        || if position.in_check() { -MATE } else { 0 },
        |(_, score)| score,
    );
    if position.turn == Color::White {
        score
    } else {
        -score
    }
}

/// The best move within `budget` and its score for the side to move.
fn search(position: &Position, budget: Duration) -> Option<(Move, i32)> {
    let mut search = Search {
        deadline: Instant::now() + budget,
        nodes: 0,
        stopped: false,
    };
    let mut moves = position.legal_moves();
    let mut best = (moves.first().copied()?, 0);
    // Each depth starts from the previous order, best first, so that a search
    // cut short by the deadline still improves on the shallower answer.
    for depth in 1..=64 {
        let mut scored = Vec::with_capacity(moves.len());
        let mut alpha = -MATE - 1;
        for &mv in &moves {
            let score = -search.negamax(&position.after(mv), depth - 1, -MATE - 1, -alpha, 1);
            if search.stopped {
                break;
            }
            alpha = alpha.max(score);
            scored.push((score, mv));
        }
        if search.stopped {
            break;
        }
        scored.sort_by_key(|&(score, _)| -score);
        best = (scored[0].1, scored[0].0);
        let mating = scored[0].0 >= MATE - 100;
        moves = scored.into_iter().map(|(_, mv)| mv).collect();
        if mating {
            break;
        }
    }
    Some(best)
}

struct Search {
    deadline: Instant,
    nodes: u64,
    stopped: bool,
}

impl Search {
    /// Check the clock now and then; a search past its deadline unwinds with
    /// scores the caller discards.
    fn out_of_time(&mut self) -> bool {
        self.nodes += 1;
        if self.nodes.is_multiple_of(512) && Instant::now() >= self.deadline {
            self.stopped = true;
        }
        self.stopped
    }

    fn negamax(&mut self, position: &Position, depth: u32, alpha: i32, beta: i32, ply: i32) -> i32 {
        if self.out_of_time() {
            return 0;
        }
        let mut moves = position.legal_moves();
        if moves.is_empty() {
            return if position.in_check() { -MATE + ply } else { 0 };
        }
        if position.halfmove() >= 100 {
            return 0;
        }
        if depth == 0 {
            return self.quiesce(position, alpha, beta, ply, 0);
        }
        order(position, &mut moves);
        let mut alpha = alpha;
        let mut best = -MATE - 1;
        for mv in moves {
            let score = -self.negamax(&position.after(mv), depth - 1, -beta, -alpha, ply + 1);
            if self.stopped {
                return 0;
            }
            best = best.max(score);
            alpha = alpha.max(score);
            if alpha >= beta {
                break;
            }
        }
        best
    }

    /// Settle captures before trusting an evaluation, so a leaf never scores
    /// a queen as won when it is about to be taken back.
    fn quiesce(&mut self, position: &Position, alpha: i32, beta: i32, ply: i32, plies: u32) -> i32 {
        if self.out_of_time() {
            return 0;
        }
        let moves = position.legal_moves();
        if moves.is_empty() {
            return if position.in_check() { -MATE + ply } else { 0 };
        }
        let stand = evaluate(position);
        if stand >= beta || plies >= QUIET_PLIES {
            return stand;
        }
        let mut alpha = alpha.max(stand);
        let mut captures: Vec<Move> = moves
            .into_iter()
            .filter(|&mv| gain(position, mv) > 0)
            .collect();
        order(position, &mut captures);
        for mv in captures {
            let score = -self.quiesce(&position.after(mv), -beta, -alpha, ply + 1, plies + 1);
            if self.stopped {
                return 0;
            }
            if score >= beta {
                return score;
            }
            alpha = alpha.max(score);
        }
        alpha
    }
}

/// Most valuable victim first, taken by the least valuable attacker.
fn order(position: &Position, moves: &mut [Move]) {
    moves.sort_by_key(|&mv| -gain(position, mv));
}

/// What a move wins outright: the victim, and the promotion piece.
fn gain(position: &Position, mv: Move) -> i32 {
    let mover = position.piece(mv.from).map_or(0, |piece| value(piece.kind));
    let victim = match position.piece(mv.to) {
        Some(piece) => value(piece.kind),
        // A pawn leaving its file without a victim on the square takes en passant.
        None if mover == value(Kind::Pawn) && mv.to.file() != mv.from.file() => value(Kind::Pawn),
        None => 0,
    };
    let promotion = mv
        .promotion
        .map_or(0, |kind| value(kind) - value(Kind::Pawn));
    if victim == 0 {
        promotion
    } else {
        victim * 10 - mover + promotion
    }
}

fn value(kind: Kind) -> i32 {
    match kind {
        Kind::Pawn => 100,
        Kind::Knight => 320,
        Kind::Bishop => 330,
        Kind::Rook => 500,
        Kind::Queen => 900,
        Kind::King => 0,
    }
}

/// Material and placement from the side to move's point of view.
fn evaluate(position: &Position) -> i32 {
    let pieces: Vec<_> = Square::all()
        .filter_map(|square| position.piece(square).map(|piece| (square, piece)))
        .collect();
    let endgame = pieces.iter().all(|(_, piece)| piece.kind != Kind::Queen);
    let mut score = 0;
    for &(square, piece) in &pieces {
        // The tables read from White's side; a black piece looks up its mirror.
        let row = if piece.color == Color::White {
            7 - square.rank()
        } else {
            square.rank()
        };
        let table = match piece.kind {
            Kind::Pawn => &PAWN,
            Kind::Knight => &KNIGHT,
            Kind::Bishop => &BISHOP,
            Kind::Rook => &ROOK,
            Kind::Queen => &QUEEN,
            Kind::King if endgame => &KING_ENDGAME,
            Kind::King => &KING,
        };
        let worth = value(piece.kind) + table[usize::from(row * 8 + square.file())];
        score += if piece.color == position.turn {
            worth
        } else {
            -worth
        };
    }
    score
}

// Tomasz Michniewski's simplified evaluation tables, rank 8 first.
#[rustfmt::skip]
const PAWN: [i32; 64] = [
     0,  0,  0,  0,  0,  0,  0,  0,
    50, 50, 50, 50, 50, 50, 50, 50,
    10, 10, 20, 30, 30, 20, 10, 10,
     5,  5, 10, 25, 25, 10,  5,  5,
     0,  0,  0, 20, 20,  0,  0,  0,
     5, -5,-10,  0,  0,-10, -5,  5,
     5, 10, 10,-20,-20, 10, 10,  5,
     0,  0,  0,  0,  0,  0,  0,  0,
];
#[rustfmt::skip]
const KNIGHT: [i32; 64] = [
    -50,-40,-30,-30,-30,-30,-40,-50,
    -40,-20,  0,  0,  0,  0,-20,-40,
    -30,  0, 10, 15, 15, 10,  0,-30,
    -30,  5, 15, 20, 20, 15,  5,-30,
    -30,  0, 15, 20, 20, 15,  0,-30,
    -30,  5, 10, 15, 15, 10,  5,-30,
    -40,-20,  0,  5,  5,  0,-20,-40,
    -50,-40,-30,-30,-30,-30,-40,-50,
];
#[rustfmt::skip]
const BISHOP: [i32; 64] = [
    -20,-10,-10,-10,-10,-10,-10,-20,
    -10,  0,  0,  0,  0,  0,  0,-10,
    -10,  0,  5, 10, 10,  5,  0,-10,
    -10,  5,  5, 10, 10,  5,  5,-10,
    -10,  0, 10, 10, 10, 10,  0,-10,
    -10, 10, 10, 10, 10, 10, 10,-10,
    -10,  5,  0,  0,  0,  0,  5,-10,
    -20,-10,-10,-10,-10,-10,-10,-20,
];
#[rustfmt::skip]
const ROOK: [i32; 64] = [
     0,  0,  0,  0,  0,  0,  0,  0,
     5, 10, 10, 10, 10, 10, 10,  5,
    -5,  0,  0,  0,  0,  0,  0, -5,
    -5,  0,  0,  0,  0,  0,  0, -5,
    -5,  0,  0,  0,  0,  0,  0, -5,
    -5,  0,  0,  0,  0,  0,  0, -5,
    -5,  0,  0,  0,  0,  0,  0, -5,
     0,  0,  0,  5,  5,  0,  0,  0,
];
#[rustfmt::skip]
const QUEEN: [i32; 64] = [
    -20,-10,-10, -5, -5,-10,-10,-20,
    -10,  0,  0,  0,  0,  0,  0,-10,
    -10,  0,  5,  5,  5,  5,  0,-10,
     -5,  0,  5,  5,  5,  5,  0, -5,
      0,  0,  5,  5,  5,  5,  0, -5,
    -10,  5,  5,  5,  5,  5,  0,-10,
    -10,  0,  5,  0,  0,  0,  0,-10,
    -20,-10,-10, -5, -5,-10,-10,-20,
];
#[rustfmt::skip]
const KING: [i32; 64] = [
    -30,-40,-40,-50,-50,-40,-40,-30,
    -30,-40,-40,-50,-50,-40,-40,-30,
    -30,-40,-40,-50,-50,-40,-40,-30,
    -30,-40,-40,-50,-50,-40,-40,-30,
    -20,-30,-30,-40,-40,-30,-30,-20,
    -10,-20,-20,-20,-20,-20,-20,-10,
     20, 20,  0,  0,  0,  0, 20, 20,
     20, 30, 10,  0,  0, 10, 30, 20,
];
#[rustfmt::skip]
const KING_ENDGAME: [i32; 64] = [
    -50,-40,-30,-20,-20,-30,-40,-50,
    -30,-20,-10,  0,  0,-10,-20,-30,
    -30,-10, 20, 30, 30, 20,-10,-30,
    -30,-10, 30, 40, 40, 30,-10,-30,
    -30,-10, 30, 40, 40, 30,-10,-30,
    -30,-10, 20, 30, 30, 20,-10,-30,
    -30,-30,  0,  0,  0,  0,-30,-30,
    -50,-30,-30,-30,-30,-30,-30,-50,
];
