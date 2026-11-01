//! Static evaluation.
//!
//! Material plus piece-square tables. The tables are the classic "small
//! positional nudges" shape: knights want the centre, pawns want to advance,
//! the king wants to hide behind its pawns. They are written the way a board is
//! usually drawn (rank 8 first) and mirrored for black, so a single table
//! serves both colours.
//!
//! All scores are centipawns from White's point of view inside
//! [`evaluate_white`], and from the side to move in [`evaluate`], which is what
//! negamax expects.

use crate::position::Position;
use crate::types::{Color, PieceType, Square};

pub const PAWN_VALUE: i32 = 100;
pub const KNIGHT_VALUE: i32 = 320;
pub const BISHOP_VALUE: i32 = 335;
pub const ROOK_VALUE: i32 = 500;
pub const QUEEN_VALUE: i32 = 975;
pub const KING_VALUE: i32 = 0;

/// Having both bishops is worth a little more than the sum of its parts.
pub const BISHOP_PAIR_BONUS: i32 = 30;
/// A small bonus for having the move, so the engine does not shuffle.
pub const TEMPO_BONUS: i32 = 10;

#[inline]
pub const fn piece_value(kind: PieceType) -> i32 {
    match kind {
        PieceType::Pawn => PAWN_VALUE,
        PieceType::Knight => KNIGHT_VALUE,
        PieceType::Bishop => BISHOP_VALUE,
        PieceType::Rook => ROOK_VALUE,
        PieceType::Queen => QUEEN_VALUE,
        PieceType::King => KING_VALUE,
    }
}

/// Tables are stored rank 8 first; a1 is the last entry.
#[inline]
const fn table_index(square: Square) -> usize {
    (7 - square.rank()) as usize * 8 + square.file() as usize
}

#[inline]
fn table_value(table: &[i32; 64], color: Color, square: Square) -> i32 {
    match color {
        Color::White => table[table_index(square)],
        Color::Black => table[table_index(square.flip_rank())],
    }
}

const PAWN_TABLE: [i32; 64] = [
    0, 0, 0, 0, 0, 0, 0, 0, //
    50, 50, 50, 50, 50, 50, 50, 50, //
    10, 10, 20, 30, 30, 20, 10, 10, //
    5, 5, 10, 25, 25, 10, 5, 5, //
    0, 0, 0, 20, 20, 0, 0, 0, //
    5, -5, -10, 0, 0, -10, -5, 5, //
    5, 10, 10, -20, -20, 10, 10, 5, //
    0, 0, 0, 0, 0, 0, 0, 0, //
];

const KNIGHT_TABLE: [i32; 64] = [
    -50, -40, -30, -30, -30, -30, -40, -50, //
    -40, -20, 0, 0, 0, 0, -20, -40, //
    -30, 0, 10, 15, 15, 10, 0, -30, //
    -30, 5, 15, 20, 20, 15, 5, -30, //
    -30, 0, 15, 20, 20, 15, 0, -30, //
    -30, 5, 10, 15, 15, 10, 5, -30, //
    -40, -20, 0, 5, 5, 0, -20, -40, //
    -50, -40, -30, -30, -30, -30, -40, -50, //
];

const BISHOP_TABLE: [i32; 64] = [
    -20, -10, -10, -10, -10, -10, -10, -20, //
    -10, 0, 0, 0, 0, 0, 0, -10, //
    -10, 0, 5, 10, 10, 5, 0, -10, //
    -10, 5, 5, 10, 10, 5, 5, -10, //
    -10, 0, 10, 10, 10, 10, 0, -10, //
    -10, 10, 10, 10, 10, 10, 10, -10, //
    -10, 5, 0, 0, 0, 0, 5, -10, //
    -20, -10, -10, -10, -10, -10, -10, -20, //
];

const ROOK_TABLE: [i32; 64] = [
    0, 0, 0, 0, 0, 0, 0, 0, //
    10, 15, 15, 15, 15, 15, 15, 10, //
    -5, 0, 0, 0, 0, 0, 0, -5, //
    -5, 0, 0, 0, 0, 0, 0, -5, //
    -5, 0, 0, 0, 0, 0, 0, -5, //
    -5, 0, 0, 0, 0, 0, 0, -5, //
    -5, 0, 0, 0, 0, 0, 0, -5, //
    0, 0, 5, 10, 10, 5, 0, 0, //
];

const QUEEN_TABLE: [i32; 64] = [
    -20, -10, -10, -5, -5, -10, -10, -20, //
    -10, 0, 0, 0, 0, 0, 0, -10, //
    -10, 0, 5, 5, 5, 5, 0, -10, //
    -5, 0, 5, 5, 5, 5, 0, -5, //
    0, 0, 5, 5, 5, 5, 0, -5, //
    -10, 5, 5, 5, 5, 5, 0, -10, //
    -10, 0, 5, 0, 0, 0, 0, -10, //
    -20, -10, -10, -5, -5, -10, -10, -20, //
];

/// Middlegame king table: stay home, keep the pawn shield intact.
const KING_TABLE: [i32; 64] = [
    -30, -40, -40, -50, -50, -40, -40, -30, //
    -30, -40, -40, -50, -50, -40, -40, -30, //
    -30, -40, -40, -50, -50, -40, -40, -30, //
    -30, -40, -40, -50, -50, -40, -40, -30, //
    -20, -30, -30, -40, -40, -30, -30, -20, //
    -10, -20, -20, -20, -20, -20, -20, -10, //
    20, 20, 0, 0, 0, 0, 20, 20, //
    20, 30, 10, 0, 0, 10, 30, 20, //
];

const fn piece_table(kind: PieceType) -> &'static [i32; 64] {
    match kind {
        PieceType::Pawn => &PAWN_TABLE,
        PieceType::Knight => &KNIGHT_TABLE,
        PieceType::Bishop => &BISHOP_TABLE,
        PieceType::Rook => &ROOK_TABLE,
        PieceType::Queen => &QUEEN_TABLE,
        PieceType::King => &KING_TABLE,
    }
}

/// Evaluate from White's point of view. Used by the GUI's evaluation bar.
pub fn evaluate_white(position: &Position) -> i32 {
    let mut score = 0;

    for color in Color::ALL {
        let sign = if color == Color::White { 1 } else { -1 };
        let occupancy = position.color_occupancy(color);

        for kind in PieceType::ALL {
            let table = piece_table(kind);
            for square in position.pieces(kind) & occupancy {
                score += sign * (piece_value(kind) + table_value(table, color, square));
            }
        }

        if (position.pieces(PieceType::Bishop) & occupancy).count() >= 2 {
            score += sign * BISHOP_PAIR_BONUS;
        }
    }

    score
}

/// Evaluate from the side to move's point of view, as negamax needs.
pub fn evaluate(position: &Position) -> i32 {
    let white = evaluate_white(position);
    match position.side_to_move() {
        Color::White => white + TEMPO_BONUS,
        Color::Black => -white + TEMPO_BONUS,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::position::STARTPOS_FEN;

    #[test]
    fn starting_position_is_balanced() {
        let position = Position::startpos();
        assert_eq!(evaluate_white(&position), 0);
        // Tempo gives the side to move a nudge.
        assert_eq!(evaluate(&position), TEMPO_BONUS);

        let mut black_to_move = Position::from_fen(&STARTPOS_FEN.replace(" w ", " b ")).unwrap();
        black_to_move = Position::from_fen(&black_to_move.to_fen()).unwrap();
        assert_eq!(evaluate(&black_to_move), TEMPO_BONUS);
    }

    #[test]
    fn material_dominates() {
        let up_a_queen = Position::from_fen("4k3/8/8/8/8/8/8/3QK3 w - - 0 1").unwrap();
        assert!(evaluate_white(&up_a_queen) > QUEEN_VALUE - 100);

        let down_a_rook = Position::from_fen("4k3/8/8/8/8/8/8/4K3 w - - 0 1").unwrap();
        let with_rook = Position::from_fen("4k3/8/8/8/8/8/8/R3K3 w - - 0 1").unwrap();
        assert!(evaluate_white(&with_rook) > evaluate_white(&down_a_rook));
    }

    #[test]
    fn tables_mirror_between_colours() {
        // A white knight on d4 and a black knight on d5 occupy mirrored
        // squares, so their positional scores must mirror too.
        let white = Position::from_fen("4k3/8/8/8/3N4/8/8/4K3 w - - 0 1").unwrap();
        let black = Position::from_fen("4k3/8/8/3n4/8/8/8/4K3 w - - 0 1").unwrap();
        assert_eq!(evaluate_white(&white), -evaluate_white(&black));
    }

    #[test]
    fn bishop_pair_is_a_bonus() {
        let one = Position::from_fen("4k3/8/8/8/8/8/8/2B1K3 w - - 0 1").unwrap();
        let two = Position::from_fen("4k3/8/8/8/8/8/8/2B1KB2 w - - 0 1").unwrap();
        assert!(evaluate_white(&two) - evaluate_white(&one) > BISHOP_VALUE);
    }
}
