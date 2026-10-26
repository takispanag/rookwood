//! Attack tables.
//!
//! Leapers (pawns, knights, kings) use small precomputed tables. Sliding
//! pieces use *ray masks*: for every square and direction we precompute the
//! squares in that direction, then clip the ray at the first blocker with a
//! single `trailing_zeros` / `leading_zeros`. This keeps the code short and
//! avoids magic numbers entirely, at a small cost in speed compared to magic
//! bitboards.

use crate::bitboard::Bitboard;
use crate::types::{Color, Square};

// Direction indices. The first four increase the square index, the last four
// decrease it; `ray_attack` relies on that ordering to pick the right end of
// the ray when it looks for the nearest blocker.
pub const NORTH: usize = 0;
pub const NORTH_EAST: usize = 1;
pub const EAST: usize = 2;
pub const NORTH_WEST: usize = 3;
pub const SOUTH: usize = 4;
pub const SOUTH_WEST: usize = 5;
pub const WEST: usize = 6;
pub const SOUTH_EAST: usize = 7;

const DIRECTIONS: [(i8, i8); 8] = [
    (0, 1),
    (1, 1),
    (1, 0),
    (-1, 1),
    (0, -1),
    (-1, -1),
    (-1, 0),
    (1, -1),
];

const KNIGHT_OFFSETS: [(i8, i8); 8] = [
    (1, 2),
    (2, 1),
    (2, -1),
    (1, -2),
    (-1, -2),
    (-2, -1),
    (-2, 1),
    (-1, 2),
];

const KING_OFFSETS: [(i8, i8); 8] = [
    (1, 0),
    (1, 1),
    (0, 1),
    (-1, 1),
    (-1, 0),
    (-1, -1),
    (0, -1),
    (1, -1),
];

#[inline]
const fn file_of(square: usize) -> i8 {
    (square % 8) as i8
}

#[inline]
const fn rank_of(square: usize) -> i8 {
    (square / 8) as i8
}

/// All squares strictly beyond `square` in direction `dir`.
const fn ray(dir: usize, square: usize) -> u64 {
    let (df, dr) = DIRECTIONS[dir];
    let mut file = file_of(square) + df;
    let mut rank = rank_of(square) + dr;
    let mut bits = 0u64;
    while file >= 0 && file < 8 && rank >= 0 && rank < 8 {
        bits |= 1u64 << ((rank as usize) * 8 + file as usize);
        file += df;
        rank += dr;
    }
    bits
}

/// `RAYS[direction][square]`.
pub const RAYS: [[Bitboard; 64]; 8] = {
    let mut table = [[Bitboard(0); 64]; 8];
    let mut dir = 0;
    while dir < 8 {
        let mut square = 0;
        while square < 64 {
            table[dir][square] = Bitboard(ray(dir, square));
            square += 1;
        }
        dir += 1;
    }
    table
};

const fn leaper_targets(offsets: [(i8, i8); 8], square: usize) -> u64 {
    let mut bits = 0u64;
    let mut i = 0;
    while i < 8 {
        let (df, dr) = offsets[i];
        // `(0, 0)` is padding used by the pawn tables, which only need two
        // directions but share this helper. Never add the square itself.
        if df != 0 || dr != 0 {
            let file = file_of(square) + df;
            let rank = rank_of(square) + dr;
            if file >= 0 && file < 8 && rank >= 0 && rank < 8 {
                bits |= 1u64 << ((rank as usize) * 8 + file as usize);
            }
        }
        i += 1;
    }
    bits
}

const fn leaper_table(offsets: [(i8, i8); 8]) -> [Bitboard; 64] {
    let mut table = [Bitboard(0); 64];
    let mut square = 0;
    while square < 64 {
        table[square] = Bitboard(leaper_targets(offsets, square));
        square += 1;
    }
    table
}

pub const KNIGHT_ATTACKS: [Bitboard; 64] = leaper_table(KNIGHT_OFFSETS);
pub const KING_ATTACKS: [Bitboard; 64] = leaper_table(KING_OFFSETS);

/// `PAWN_ATTACKS[colour][square]`: the two diagonal squares a pawn covers.
pub const PAWN_ATTACKS: [[Bitboard; 64]; 2] = {
    let white = leaper_table([
        (1, 1),
        (-1, 1),
        (0, 0),
        (0, 0),
        (0, 0),
        (0, 0),
        (0, 0),
        (0, 0),
    ]);
    let black = leaper_table([
        (1, -1),
        (-1, -1),
        (0, 0),
        (0, 0),
        (0, 0),
        (0, 0),
        (0, 0),
        (0, 0),
    ]);
    [white, black]
};

#[inline]
pub const fn knight_attacks(square: Square) -> Bitboard {
    KNIGHT_ATTACKS[square.index()]
}

#[inline]
pub const fn king_attacks(square: Square) -> Bitboard {
    KING_ATTACKS[square.index()]
}

#[inline]
pub const fn pawn_attacks(color: Color, square: Square) -> Bitboard {
    PAWN_ATTACKS[color.index()][square.index()]
}

/// Clip a single ray at its first blocker.
///
/// `ray ^ ray_from_blocker` is exactly "the ray up to and including the
/// blocker", because the two masks share a prefix.
#[inline]
fn ray_attack(square: usize, occupancy: u64, dir: usize) -> u64 {
    let ray = RAYS[dir][square].0;
    let blockers = ray & occupancy;
    if blockers == 0 {
        return ray;
    }
    let blocker = if dir <= NORTH_WEST {
        blockers.trailing_zeros()
    } else {
        63 - blockers.leading_zeros()
    };
    ray ^ RAYS[dir][blocker as usize].0
}

#[inline]
pub fn rook_attacks(square: Square, occupancy: Bitboard) -> Bitboard {
    let sq = square.index();
    let occ = occupancy.0;
    Bitboard(
        ray_attack(sq, occ, NORTH)
            | ray_attack(sq, occ, EAST)
            | ray_attack(sq, occ, SOUTH)
            | ray_attack(sq, occ, WEST),
    )
}

#[inline]
pub fn bishop_attacks(square: Square, occupancy: Bitboard) -> Bitboard {
    let sq = square.index();
    let occ = occupancy.0;
    Bitboard(
        ray_attack(sq, occ, NORTH_EAST)
            | ray_attack(sq, occ, NORTH_WEST)
            | ray_attack(sq, occ, SOUTH_EAST)
            | ray_attack(sq, occ, SOUTH_WEST),
    )
}

#[inline]
pub fn queen_attacks(square: Square, occupancy: Bitboard) -> Bitboard {
    rook_attacks(square, occupancy) | bishop_attacks(square, occupancy)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Straightforward reference implementation used to validate the rays.
    fn naive(square: Square, occupancy: Bitboard, directions: &[(i8, i8)]) -> Bitboard {
        let mut attacks = Bitboard::EMPTY;
        for &(df, dr) in directions {
            let mut current = square;
            while let Some(next) = current.offset(df, dr) {
                attacks = attacks.with(next);
                if occupancy.is_set(next) {
                    break;
                }
                current = next;
            }
        }
        attacks
    }

    const ORTHOGONAL: [(i8, i8); 4] = [(0, 1), (1, 0), (0, -1), (-1, 0)];
    const DIAGONAL: [(i8, i8); 4] = [(1, 1), (-1, 1), (1, -1), (-1, -1)];

    /// A small deterministic generator, so the test needs no dependencies.
    fn demo_positions() -> Vec<Bitboard> {
        let mut state = 0x1234_5678_9abc_def0u64;
        let mut next = || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        let mut positions = vec![
            Bitboard::EMPTY,
            Bitboard::FULL,
            Bitboard::FULL.with(Square::from_uci("d4").unwrap()),
        ];
        for _ in 0..64 {
            let mut occupancy = 0u64;
            for _ in 0..12 {
                occupancy |= 1u64 << (next() % 64);
            }
            positions.push(Bitboard(occupancy));
        }
        positions
    }

    #[test]
    fn sliding_attacks_match_naive_walk() {
        for occupancy in demo_positions() {
            for index in 0..64u8 {
                let square = Square::from_index(index);
                assert_eq!(
                    rook_attacks(square, occupancy),
                    naive(square, occupancy, &ORTHOGONAL),
                    "rook on {square} with occupancy {occupancy:?}"
                );
                assert_eq!(
                    bishop_attacks(square, occupancy),
                    naive(square, occupancy, &DIAGONAL),
                    "bishop on {square} with occupancy {occupancy:?}"
                );
                assert_eq!(
                    queen_attacks(square, occupancy),
                    rook_attacks(square, occupancy) | bishop_attacks(square, occupancy)
                );
            }
        }
    }

    #[test]
    fn leaper_tables_have_expected_shape() {
        assert_eq!(knight_attacks(Square::from_uci("a1").unwrap()).count(), 2);
        assert_eq!(knight_attacks(Square::from_uci("d4").unwrap()).count(), 8);
        assert_eq!(king_attacks(Square::from_uci("h8").unwrap()).count(), 3);
        assert_eq!(king_attacks(Square::from_uci("e4").unwrap()).count(), 8);
        assert_eq!(
            pawn_attacks(Color::White, Square::from_uci("e4").unwrap()).count(),
            2
        );
        assert_eq!(
            pawn_attacks(Color::Black, Square::from_uci("e4").unwrap()).count(),
            2
        );
        assert_eq!(
            pawn_attacks(Color::White, Square::from_uci("a7").unwrap()).count(),
            1
        );
        assert_eq!(
            pawn_attacks(Color::Black, Square::from_uci("h2").unwrap()).count(),
            1
        );
    }

    #[test]
    fn pawn_attacks_point_the_right_way() {
        let white = pawn_attacks(Color::White, Square::from_uci("d4").unwrap());
        assert!(white.is_set(Square::from_uci("c5").unwrap()));
        assert!(white.is_set(Square::from_uci("e5").unwrap()));
        let black = pawn_attacks(Color::Black, Square::from_uci("d4").unwrap());
        assert!(black.is_set(Square::from_uci("c3").unwrap()));
        assert!(black.is_set(Square::from_uci("e3").unwrap()));
    }

    #[test]
    fn ray_clipping_includes_the_blocker() {
        let occupancy = Bitboard::from_square(Square::from_uci("e4").unwrap())
            | Bitboard::from_square(Square::from_uci("e6").unwrap());
        let attacks = rook_attacks(Square::from_uci("e2").unwrap(), occupancy);
        assert!(attacks.is_set(Square::from_uci("e3").unwrap()));
        assert!(attacks.is_set(Square::from_uci("e4").unwrap()));
        assert!(!attacks.is_set(Square::from_uci("e5").unwrap()));
        assert!(!attacks.is_set(Square::from_uci("e6").unwrap()));
    }
}
