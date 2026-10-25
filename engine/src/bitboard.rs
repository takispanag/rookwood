//! A 64-bit set of squares.
//!
//! Bit `n` corresponds to [`Square`] `n`, so `a1` is the least significant bit.
//! Every operation here is a single machine instruction on any 64-bit target.

use crate::types::Square;
use std::fmt;
use std::ops::{
    BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Not, Shl, Shr, Sub,
};

/// A set of squares.
#[derive(Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Bitboard(pub u64);

impl Bitboard {
    pub const EMPTY: Bitboard = Bitboard(0);
    pub const FULL: Bitboard = Bitboard(u64::MAX);
    pub const A1: Bitboard = Bitboard(1);
    pub const H8: Bitboard = Bitboard(1 << 63);

    #[inline]
    pub const fn from_square(square: Square) -> Bitboard {
        Bitboard(1u64 << square.index())
    }

    #[inline]
    pub const fn from_index(index: usize) -> Bitboard {
        Bitboard(1u64 << index)
    }

    #[inline]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    #[inline]
    pub const fn is_set(self, square: Square) -> bool {
        self.0 & (1u64 << square.index()) != 0
    }

    #[inline]
    pub const fn count(self) -> u32 {
        self.0.count_ones()
    }

    #[inline]
    pub const fn contains(self, other: Bitboard) -> bool {
        self.0 & other.0 == other.0
    }

    #[inline]
    pub const fn intersects(self, other: Bitboard) -> bool {
        self.0 & other.0 != 0
    }

    #[inline]
    pub const fn with(self, square: Square) -> Bitboard {
        Bitboard(self.0 | (1u64 << square.index()))
    }

    #[inline]
    pub const fn without(self, square: Square) -> Bitboard {
        Bitboard(self.0 & !(1u64 << square.index()))
    }

    /// The least significant set square, or `None` for an empty set.
    #[inline]
    pub const fn lsb(self) -> Option<Square> {
        if self.0 == 0 {
            None
        } else {
            Some(Square::from_index(self.0.trailing_zeros() as u8))
        }
    }

    /// Remove and return the least significant square.
    #[inline]
    pub fn pop_lsb(&mut self) -> Option<Square> {
        let square = self.lsb()?;
        self.0 &= self.0 - 1;
        Some(square)
    }

    #[inline]
    pub const fn more_than_one(self) -> bool {
        self.0 & self.0.wrapping_sub(1) != 0
    }

    /// Shift one rank towards the black side (`a2 -> a1`), dropping rank 1.
    #[inline]
    pub const fn south(self) -> Bitboard {
        Bitboard(self.0 >> 8)
    }

    /// Shift one rank towards the white side (`a1 -> a2`), dropping rank 8.
    #[inline]
    pub const fn north(self) -> Bitboard {
        Bitboard(self.0 << 8)
    }

    #[inline]
    pub const fn east(self) -> Bitboard {
        Bitboard((self.0 & !FILE_H.0) << 1)
    }

    #[inline]
    pub const fn west(self) -> Bitboard {
        Bitboard((self.0 & !FILE_A.0) >> 1)
    }

    /// Iterate the squares of the set, in ascending order.
    #[inline]
    pub const fn squares(self) -> Squares {
        Squares(self)
    }
}

/// Iterator over the squares of a [`Bitboard`].
#[derive(Clone, Copy)]
pub struct Squares(Bitboard);

impl Iterator for Squares {
    type Item = Square;

    #[inline]
    fn next(&mut self) -> Option<Square> {
        self.0.pop_lsb()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let count = self.0.count() as usize;
        (count, Some(count))
    }
}

impl ExactSizeIterator for Squares {}

impl IntoIterator for Bitboard {
    type Item = Square;
    type IntoIter = Squares;

    fn into_iter(self) -> Squares {
        self.squares()
    }
}

impl BitAnd for Bitboard {
    type Output = Bitboard;
    #[inline]
    fn bitand(self, rhs: Bitboard) -> Bitboard {
        Bitboard(self.0 & rhs.0)
    }
}

impl BitOr for Bitboard {
    type Output = Bitboard;
    #[inline]
    fn bitor(self, rhs: Bitboard) -> Bitboard {
        Bitboard(self.0 | rhs.0)
    }
}

impl BitXor for Bitboard {
    type Output = Bitboard;
    #[inline]
    fn bitxor(self, rhs: Bitboard) -> Bitboard {
        Bitboard(self.0 ^ rhs.0)
    }
}

impl Sub for Bitboard {
    type Output = Bitboard;
    #[inline]
    fn sub(self, rhs: Bitboard) -> Bitboard {
        Bitboard(self.0 & !rhs.0)
    }
}

impl BitAndAssign for Bitboard {
    #[inline]
    fn bitand_assign(&mut self, rhs: Bitboard) {
        self.0 &= rhs.0;
    }
}

impl BitOrAssign for Bitboard {
    #[inline]
    fn bitor_assign(&mut self, rhs: Bitboard) {
        self.0 |= rhs.0;
    }
}

impl BitXorAssign for Bitboard {
    #[inline]
    fn bitxor_assign(&mut self, rhs: Bitboard) {
        self.0 ^= rhs.0;
    }
}

impl Not for Bitboard {
    type Output = Bitboard;
    #[inline]
    fn not(self) -> Bitboard {
        Bitboard(!self.0)
    }
}

impl Shl<u32> for Bitboard {
    type Output = Bitboard;
    #[inline]
    fn shl(self, rhs: u32) -> Bitboard {
        Bitboard(self.0 << rhs)
    }
}

impl Shr<u32> for Bitboard {
    type Output = Bitboard;
    #[inline]
    fn shr(self, rhs: u32) -> Bitboard {
        Bitboard(self.0 >> rhs)
    }
}

impl From<Bitboard> for u64 {
    fn from(board: Bitboard) -> u64 {
        board.0
    }
}

/// Render as an ASCII board, rank 8 first. Used by tests and `d` in the UCI loop.
impl fmt::Display for Bitboard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for rank in (0..8).rev() {
            write!(f, "{} ", rank + 1)?;
            for file in 0..8 {
                let bit = (self.0 >> (rank * 8 + file)) & 1;
                write!(f, "{} ", if bit == 1 { 'X' } else { '.' })?;
            }
            writeln!(f)?;
        }
        write!(f, "  a b c d e f g h")
    }
}

impl fmt::Debug for Bitboard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Bitboard(0x{:016x})", self.0)
    }
}

macro_rules! file_constants {
    ($($name:ident => $file:literal),* $(,)?) => {
        $(
            pub const $name: Bitboard = Bitboard(0x0101_0101_0101_0101u64 << $file);
        )*
    };
}

file_constants! {
    FILE_A => 0,
    FILE_B => 1,
    FILE_C => 2,
    FILE_D => 3,
    FILE_E => 4,
    FILE_F => 5,
    FILE_G => 6,
    FILE_H => 7,
}

pub const RANK_1: Bitboard = Bitboard(0x0000_0000_0000_00ff);
pub const RANK_2: Bitboard = Bitboard(0x0000_0000_0000_ff00);
pub const RANK_3: Bitboard = Bitboard(0x0000_0000_00ff_0000);
pub const RANK_4: Bitboard = Bitboard(0x0000_0000_ff00_0000);
pub const RANK_5: Bitboard = Bitboard(0x0000_00ff_0000_0000);
pub const RANK_6: Bitboard = Bitboard(0x0000_ff00_0000_0000);
pub const RANK_7: Bitboard = Bitboard(0x00ff_0000_0000_0000);
pub const RANK_8: Bitboard = Bitboard(0xff00_0000_0000_0000);

/// The four central squares, handy for evaluation and tests.
pub const CENTER: Bitboard = Bitboard(0x0000_0000_1818_0000);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_set_operations() {
        let mut board = Bitboard::EMPTY.with(Square::from_uci("e4").unwrap());
        assert_eq!(board.count(), 1);
        assert!(board.is_set(Square::from_uci("e4").unwrap()));
        board = board.with(Square::from_uci("h8").unwrap());
        assert_eq!(board.count(), 2);

        let collected: Vec<Square> = board.squares().collect();
        assert_eq!(collected.len(), 2);
        assert_eq!(collected[0].to_uci(), "e4");
        assert_eq!(collected[1].to_uci(), "h8");
    }

    #[test]
    fn counting_ranks_and_files() {
        assert_eq!(RANK_1.count(), 8);
        assert_eq!(FILE_A.count(), 8);
        assert_eq!((RANK_4 & FILE_E).count(), 1);
        assert_eq!(Bitboard::FULL.count(), 64);
        assert_eq!(Bitboard::EMPTY.count(), 0);
    }

    #[test]
    fn edge_shifts_drop_off_board_squares() {
        let a_file = FILE_A;
        assert_eq!(a_file.west(), Bitboard::EMPTY);
        assert_eq!(FILE_H.east(), Bitboard::EMPTY);
        assert_eq!(RANK_1.south(), Bitboard::EMPTY);
        assert_eq!(
            (Bitboard::from_square(Square::from_uci("b2").unwrap()))
                .south()
                .count(),
            1
        );
    }

    #[test]
    fn pop_lsb_yields_ascending_squares() {
        let board = FILE_A | RANK_8;
        let mut count = 0;
        let mut last = None;
        for square in board {
            if let Some(previous) = last {
                assert!(square.index() > previous);
            }
            last = Some(square.index());
            count += 1;
        }
        assert_eq!(count, 15);
    }
}
