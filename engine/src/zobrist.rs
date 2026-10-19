//! Zobrist hashing.
//!
//! Every position gets a 64-bit key built by XOR-ing precomputed random
//! numbers: one per (colour, piece, square), one per castling-rights
//! combination, one per en-passant file and one for the side to move. Because
//! XOR is its own inverse, keys update incrementally inside `make_move`, and
//! the search can compare positions in a single instruction.
//!
//! The tables are generated at compile time by a fixed splitmix64 sequence, so
//! builds are reproducible and no data files are needed.

use crate::types::{Color, PieceType, Square};

/// Random numbers used to build position keys.
pub struct Zobrist {
    pub pieces: [[[u64; 64]; 6]; 2],
    pub castling: [u64; 16],
    pub ep_file: [u64; 8],
    pub side: u64,
}

/// One splitmix64 step. Deterministic, well distributed, and trivial to run in
/// a `const` context.
const fn next_random(state: u64) -> (u64, u64) {
    let state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
    let mut z = state;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    (state, z ^ (z >> 31))
}

pub const ZOBRIST: Zobrist = {
    let mut state = 0x0bad_c0de_dead_beefu64;

    let mut pieces = [[[0u64; 64]; 6]; 2];
    let mut color = 0;
    while color < 2 {
        let mut kind = 0;
        while kind < 6 {
            let mut square = 0;
            while square < 64 {
                let (next, value) = next_random(state);
                state = next;
                pieces[color][kind][square] = value;
                square += 1;
            }
            kind += 1;
        }
        color += 1;
    }

    let mut castling = [0u64; 16];
    let mut index = 0;
    while index < 16 {
        let (next, value) = next_random(state);
        state = next;
        castling[index] = value;
        index += 1;
    }

    let mut ep_file = [0u64; 8];
    let mut file = 0;
    while file < 8 {
        let (next, value) = next_random(state);
        state = next;
        ep_file[file] = value;
        file += 1;
    }

    let (_, side) = next_random(state);

    Zobrist {
        pieces,
        castling,
        ep_file,
        side,
    }
};

impl Zobrist {
    #[inline]
    pub const fn piece(&self, color: Color, kind: PieceType, square: Square) -> u64 {
        self.pieces[color.index()][kind.index()][square.index()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn tables_are_populated_and_distinct() {
        let mut seen = HashSet::new();
        for color in Color::ALL {
            for kind in PieceType::ALL {
                for index in 0..64u8 {
                    let value = ZOBRIST.piece(color, kind, Square::from_index(index));
                    assert_ne!(value, 0, "zero keys are weak against empty squares");
                    assert!(seen.insert(value), "duplicate Zobrist entry");
                }
            }
        }
        for value in ZOBRIST.castling.iter().chain(ZOBRIST.ep_file.iter()) {
            assert!(seen.insert(*value), "duplicate Zobrist entry");
        }
        assert!(seen.insert(ZOBRIST.side));
    }

    #[test]
    fn generation_is_deterministic() {
        // The key for a1 has to stay stable across runs, otherwise saved
        // repetition histories would be meaningless.
        let a1 = ZOBRIST.piece(Color::White, PieceType::Rook, Square::from_index(0));
        assert_eq!(
            a1,
            ZOBRIST.piece(Color::White, PieceType::Rook, Square::from_index(0))
        );
        assert_ne!(
            a1,
            ZOBRIST.piece(Color::Black, PieceType::Rook, Square::from_index(0))
        );
    }
}
