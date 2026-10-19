//! Core chess vocabulary: colours, piece kinds, squares.
//!
//! Squares use the classic little-endian rank-file numbering: `a1 = 0`,
//! `b1 = 1`, ... `h1 = 7`, `a2 = 8`, ... `h8 = 63`. A square's index is
//! therefore `rank * 8 + file`, which is also the bit position used by
//! [`crate::bitboard::Bitboard`]. Keeping both representations on the same
//! numbering means conversions are free.

use std::fmt;

/// The two players.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Color {
    White = 0,
    Black = 1,
}

impl Color {
    pub const ALL: [Color; 2] = [Color::White, Color::Black];

    #[inline]
    pub const fn index(self) -> usize {
        self as usize
    }

    #[inline]
    pub const fn from_index(index: usize) -> Color {
        debug_assert!(index < 2);
        if index == 0 {
            Color::White
        } else {
            Color::Black
        }
    }

    #[inline]
    pub const fn opposite(self) -> Color {
        match self {
            Color::White => Color::Black,
            Color::Black => Color::White,
        }
    }

    /// Direction a pawn of this colour advances in, in ranks.
    #[inline]
    pub const fn forward(self) -> i8 {
        match self {
            Color::White => 1,
            Color::Black => -1,
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Color::White => "white",
            Color::Black => "black",
        }
    }
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// A piece kind, independent of colour.
///
/// The discriminants double as indices into per-piece arrays (`pieces[6]`,
/// piece-square tables, ...), so the order must not change.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum PieceType {
    Pawn = 0,
    Knight = 1,
    Bishop = 2,
    Rook = 3,
    Queen = 4,
    King = 5,
}

impl PieceType {
    pub const ALL: [PieceType; 6] = [
        PieceType::Pawn,
        PieceType::Knight,
        PieceType::Bishop,
        PieceType::Rook,
        PieceType::Queen,
        PieceType::King,
    ];

    #[inline]
    pub const fn index(self) -> usize {
        self as usize
    }

    #[inline]
    pub const fn from_index(index: usize) -> PieceType {
        match index {
            0 => PieceType::Pawn,
            1 => PieceType::Knight,
            2 => PieceType::Bishop,
            3 => PieceType::Rook,
            4 => PieceType::Queen,
            5 => PieceType::King,
            _ => panic!("piece index out of range"),
        }
    }

    /// Upper-case piece letter, as used by FEN and SAN.
    pub const fn letter(self) -> char {
        match self {
            PieceType::Pawn => 'P',
            PieceType::Knight => 'N',
            PieceType::Bishop => 'B',
            PieceType::Rook => 'R',
            PieceType::Queen => 'Q',
            PieceType::King => 'K',
        }
    }

    pub const fn from_letter(c: char) -> Option<PieceType> {
        match c.to_ascii_uppercase() {
            'P' => Some(PieceType::Pawn),
            'N' => Some(PieceType::Knight),
            'B' => Some(PieceType::Bishop),
            'R' => Some(PieceType::Rook),
            'Q' => Some(PieceType::Queen),
            'K' => Some(PieceType::King),
            _ => None,
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            PieceType::Pawn => "pawn",
            PieceType::Knight => "knight",
            PieceType::Bishop => "bishop",
            PieceType::Rook => "rook",
            PieceType::Queen => "queen",
            PieceType::King => "king",
        }
    }
}

impl fmt::Display for PieceType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// A coloured piece.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Piece {
    pub color: Color,
    pub kind: PieceType,
}

impl Piece {
    #[inline]
    pub const fn new(color: Color, kind: PieceType) -> Piece {
        Piece { color, kind }
    }

    /// Parse a FEN/SAN piece letter: `K` is a white king, `k` a black king.
    pub fn from_char(c: char) -> Option<Piece> {
        let color = if c.is_ascii_uppercase() {
            Color::White
        } else if c.is_ascii_lowercase() {
            Color::Black
        } else {
            return None;
        };
        PieceType::from_letter(c).map(|kind| Piece::new(color, kind))
    }

    /// The FEN letter for this piece, upper case for white.
    pub const fn to_char(self) -> char {
        match self.color {
            Color::White => self.kind.letter(),
            Color::Black => self.kind.letter().to_ascii_lowercase(),
        }
    }
}

impl fmt::Display for Piece {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_char().to_string())
    }
}

/// A square on the board, `0..64` with `a1 == 0`.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct Square(u8);

pub const FILE_NAMES: [char; 8] = ['a', 'b', 'c', 'd', 'e', 'f', 'g', 'h'];
pub const RANK_NAMES: [char; 8] = ['1', '2', '3', '4', '5', '6', '7', '8'];

impl Square {
    #[inline]
    pub const fn new(file: u8, rank: u8) -> Square {
        Square(rank * 8 + file)
    }

    #[inline]
    pub const fn from_index(index: u8) -> Square {
        debug_assert!(index < 64);
        Square(index)
    }

    /// Parse coordinate notation such as `"e4"`.
    pub fn from_uci(text: &str) -> Option<Square> {
        let bytes = text.as_bytes();
        if bytes.len() != 2 {
            return None;
        }
        let file = (bytes[0] as char).to_ascii_lowercase() as u8;
        let rank = bytes[1];
        if !(b'a'..=b'h').contains(&file) || !(b'1'..=b'8').contains(&rank) {
            return None;
        }
        Some(Square::new(file - b'a', rank - b'1'))
    }

    pub fn to_uci(self) -> String {
        format!(
            "{}{}",
            FILE_NAMES[self.file() as usize],
            RANK_NAMES[self.rank() as usize]
        )
    }

    #[inline]
    pub const fn index(self) -> usize {
        self.0 as usize
    }

    #[inline]
    pub const fn file(self) -> u8 {
        self.0 % 8
    }

    #[inline]
    pub const fn rank(self) -> u8 {
        self.0 / 8
    }

    /// True for the light squares. Used by the bishop-pair and endgame code.
    #[inline]
    pub const fn is_light(self) -> bool {
        (self.file() + self.rank()) % 2 == 1
    }

    /// The square `delta` files right and `delta` ranks up, if it is on the board.
    #[inline]
    pub fn offset(self, df: i8, dr: i8) -> Option<Square> {
        let file = self.file() as i8 + df;
        let rank = self.rank() as i8 + dr;
        if (0..8).contains(&file) && (0..8).contains(&rank) {
            Some(Square::new(file as u8, rank as u8))
        } else {
            None
        }
    }

    /// Mirror the square vertically (`a1 <-> a8`). Used to reuse white tables for black.
    #[inline]
    pub const fn flip_rank(self) -> Square {
        Square(self.0 ^ 56)
    }
}

impl fmt::Debug for Square {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_uci())
    }
}

impl fmt::Display for Square {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_uci())
    }
}

impl From<Square> for usize {
    #[inline]
    fn from(square: Square) -> usize {
        square.index()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn square_numbering_round_trips() {
        for index in 0..64u8 {
            let square = Square::from_index(index);
            assert_eq!(square.index(), index as usize);
            assert_eq!(Square::from_uci(&square.to_uci()), Some(square));
        }
        assert_eq!(Square::from_uci("a1"), Some(Square::from_index(0)));
        assert_eq!(Square::from_uci("h8"), Some(Square::from_index(63)));
        assert_eq!(Square::from_uci("e4"), Some(Square::new(4, 3)));
        assert_eq!(Square::from_uci("i9"), None);
        assert_eq!(Square::from_uci("e"), None);
    }

    #[test]
    fn square_colour_alternates() {
        assert!(Square::from_uci("h1").unwrap().is_light());
        assert!(!Square::from_uci("a1").unwrap().is_light());
        assert!(Square::from_uci("a8").unwrap().is_light());
    }

    #[test]
    fn piece_letters_round_trip() {
        for kind in PieceType::ALL {
            for color in Color::ALL {
                let piece = Piece::new(color, kind);
                assert_eq!(Piece::from_char(piece.to_char()), Some(piece));
            }
        }
        assert_eq!(Piece::from_char('x'), None);
    }
}
