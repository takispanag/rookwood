//! Move encoding and move lists.
//!
//! A move fits in 16 bits: six bits for the origin square, six for the target
//! and four for a flag describing what kind of move it is (quiet, capture,
//! castle, en passant, promotion...). Keeping moves `Copy` and pointer-free
//! means the search never allocates.

use crate::types::{PieceType, Square};
use std::fmt;
use std::ops::Deref;

/// What a move does, beyond moving a piece from A to B.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
#[repr(u8)]
pub enum MoveFlag {
    Quiet = 0,
    DoublePush = 1,
    KingCastle = 2,
    QueenCastle = 3,
    Capture = 4,
    EnPassant = 5,
    // 6 and 7 are unused, mirroring the classic encoding.
    KnightPromotion = 8,
    BishopPromotion = 9,
    RookPromotion = 10,
    QueenPromotion = 11,
    KnightPromotionCapture = 12,
    BishopPromotionCapture = 13,
    RookPromotionCapture = 14,
    QueenPromotionCapture = 15,
}

impl MoveFlag {
    /// Every flag, for exhaustive tests and tooling.
    pub const ALL: [MoveFlag; 14] = [
        MoveFlag::Quiet,
        MoveFlag::DoublePush,
        MoveFlag::KingCastle,
        MoveFlag::QueenCastle,
        MoveFlag::Capture,
        MoveFlag::EnPassant,
        MoveFlag::KnightPromotion,
        MoveFlag::BishopPromotion,
        MoveFlag::RookPromotion,
        MoveFlag::QueenPromotion,
        MoveFlag::KnightPromotionCapture,
        MoveFlag::BishopPromotionCapture,
        MoveFlag::RookPromotionCapture,
        MoveFlag::QueenPromotionCapture,
    ];

    #[inline]
    pub const fn bits(self) -> u16 {
        self as u16
    }

    #[inline]
    pub const fn from_bits(bits: u16) -> MoveFlag {
        match bits {
            0 => MoveFlag::Quiet,
            1 => MoveFlag::DoublePush,
            2 => MoveFlag::KingCastle,
            3 => MoveFlag::QueenCastle,
            4 => MoveFlag::Capture,
            5 => MoveFlag::EnPassant,
            8 => MoveFlag::KnightPromotion,
            9 => MoveFlag::BishopPromotion,
            10 => MoveFlag::RookPromotion,
            11 => MoveFlag::QueenPromotion,
            12 => MoveFlag::KnightPromotionCapture,
            13 => MoveFlag::BishopPromotionCapture,
            14 => MoveFlag::RookPromotionCapture,
            _ => MoveFlag::QueenPromotionCapture,
        }
    }

    #[inline]
    pub const fn is_capture(self) -> bool {
        matches!(
            self,
            MoveFlag::Capture
                | MoveFlag::EnPassant
                | MoveFlag::KnightPromotionCapture
                | MoveFlag::BishopPromotionCapture
                | MoveFlag::RookPromotionCapture
                | MoveFlag::QueenPromotionCapture
        )
    }

    #[inline]
    pub const fn is_promotion(self) -> bool {
        self.bits() >= MoveFlag::KnightPromotion.bits()
    }

    /// The piece a promotion turns into, if this is a promotion.
    #[inline]
    pub const fn promotion_piece(self) -> Option<PieceType> {
        match self {
            MoveFlag::KnightPromotion | MoveFlag::KnightPromotionCapture => Some(PieceType::Knight),
            MoveFlag::BishopPromotion | MoveFlag::BishopPromotionCapture => Some(PieceType::Bishop),
            MoveFlag::RookPromotion | MoveFlag::RookPromotionCapture => Some(PieceType::Rook),
            MoveFlag::QueenPromotion | MoveFlag::QueenPromotionCapture => Some(PieceType::Queen),
            _ => None,
        }
    }

    pub const fn for_promotion(kind: PieceType, is_capture: bool) -> Option<MoveFlag> {
        Some(match (kind, is_capture) {
            (PieceType::Knight, false) => MoveFlag::KnightPromotion,
            (PieceType::Bishop, false) => MoveFlag::BishopPromotion,
            (PieceType::Rook, false) => MoveFlag::RookPromotion,
            (PieceType::Queen, false) => MoveFlag::QueenPromotion,
            (PieceType::Knight, true) => MoveFlag::KnightPromotionCapture,
            (PieceType::Bishop, true) => MoveFlag::BishopPromotionCapture,
            (PieceType::Rook, true) => MoveFlag::RookPromotionCapture,
            (PieceType::Queen, true) => MoveFlag::QueenPromotionCapture,
            _ => return None,
        })
    }
}

/// A chess move.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(transparent)]
pub struct Move(u16);

impl Move {
    /// A placeholder that is never a legal move. Used for "no best move yet".
    pub const NULL: Move = Move(0);

    #[inline]
    pub const fn new(from: Square, to: Square, flag: MoveFlag) -> Move {
        Move((flag.bits() << 12) | ((to.index() as u16) << 6) | from.index() as u16)
    }

    #[inline]
    pub const fn from(self) -> Square {
        Square::from_index((self.0 & 0x3f) as u8)
    }

    #[inline]
    pub const fn to(self) -> Square {
        Square::from_index(((self.0 >> 6) & 0x3f) as u8)
    }

    #[inline]
    pub const fn flag(self) -> MoveFlag {
        MoveFlag::from_bits(self.0 >> 12)
    }

    #[inline]
    pub const fn is_null(self) -> bool {
        self.0 == 0
    }

    #[inline]
    pub const fn is_capture(self) -> bool {
        self.flag().is_capture()
    }

    #[inline]
    pub const fn is_promotion(self) -> bool {
        self.flag().is_promotion()
    }

    #[inline]
    pub const fn is_en_passant(self) -> bool {
        matches!(self.flag(), MoveFlag::EnPassant)
    }

    #[inline]
    pub const fn is_castle(self) -> bool {
        matches!(self.flag(), MoveFlag::KingCastle | MoveFlag::QueenCastle)
    }

    #[inline]
    pub const fn is_king_castle(self) -> bool {
        matches!(self.flag(), MoveFlag::KingCastle)
    }

    #[inline]
    pub const fn is_queen_castle(self) -> bool {
        matches!(self.flag(), MoveFlag::QueenCastle)
    }

    #[inline]
    pub const fn is_double_push(self) -> bool {
        matches!(self.flag(), MoveFlag::DoublePush)
    }

    #[inline]
    pub const fn promotion(self) -> Option<PieceType> {
        self.flag().promotion_piece()
    }

    /// Long algebraic notation, the format UCI uses (`e2e4`, `e7e8q`).
    pub fn to_uci(self) -> String {
        let mut text = self.from().to_uci();
        text.push_str(&self.to().to_uci());
        if let Some(kind) = self.promotion() {
            text.push(kind.letter().to_ascii_lowercase());
        }
        text
    }
}

impl fmt::Display for Move {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_uci())
    }
}

impl fmt::Debug for Move {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_uci())
    }
}

/// A fixed-capacity list of moves.
///
/// 256 entries is comfortably above the 218 legal moves of the worst known
/// position, so `push` can never overflow in practice.
#[derive(Clone)]
pub struct MoveList {
    moves: [Move; 256],
    len: usize,
}

impl MoveList {
    #[inline]
    pub const fn new() -> MoveList {
        MoveList {
            moves: [Move::NULL; 256],
            len: 0,
        }
    }

    #[inline]
    pub fn push(&mut self, mv: Move) {
        debug_assert!(self.len < 256, "move list overflow");
        self.moves[self.len] = mv;
        self.len += 1;
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[inline]
    pub fn truncate(&mut self, len: usize) {
        self.len = len.min(self.len);
    }

    #[inline]
    pub fn contains(&self, mv: Move) -> bool {
        self.iter().any(|&candidate| candidate == mv)
    }

    /// Swap two entries. Used by the search's selection sort.
    #[inline]
    pub fn swap(&mut self, a: usize, b: usize) {
        debug_assert!(a < self.len && b < self.len);
        self.moves.swap(a, b);
    }

    #[inline]
    pub fn as_slice(&self) -> &[Move] {
        &self.moves[..self.len]
    }
}

impl Default for MoveList {
    fn default() -> Self {
        MoveList::new()
    }
}

impl Deref for MoveList {
    type Target = [Move];

    #[inline]
    fn deref(&self) -> &[Move] {
        self.as_slice()
    }
}

impl fmt::Debug for MoveList {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn move_bit_packing_round_trips() {
        for flag in MoveFlag::ALL {
            let mv = Move::new(
                Square::from_uci("a1").unwrap(),
                Square::from_uci("h8").unwrap(),
                flag,
            );
            assert_eq!(mv.from(), Square::from_uci("a1").unwrap());
            assert_eq!(mv.to(), Square::from_uci("h8").unwrap());
            assert_eq!(mv.flag(), flag);
        }
    }

    #[test]
    fn flag_classification() {
        assert!(MoveFlag::Capture.is_capture());
        assert!(MoveFlag::EnPassant.is_capture());
        assert!(MoveFlag::QueenPromotionCapture.is_capture());
        assert!(!MoveFlag::Quiet.is_capture());
        assert!(!MoveFlag::Quiet.is_promotion());
        assert!(MoveFlag::RookPromotion.is_promotion());
        assert_eq!(
            MoveFlag::KnightPromotionCapture.promotion_piece(),
            Some(PieceType::Knight)
        );
        assert_eq!(MoveFlag::Quiet.promotion_piece(), None);
    }

    #[test]
    fn uci_formatting() {
        let mv = Move::new(
            Square::from_uci("e7").unwrap(),
            Square::from_uci("e8").unwrap(),
            MoveFlag::QueenPromotion,
        );
        assert_eq!(mv.to_uci(), "e7e8q");

        let castle = Move::new(
            Square::from_uci("e1").unwrap(),
            Square::from_uci("g1").unwrap(),
            MoveFlag::KingCastle,
        );
        assert_eq!(castle.to_uci(), "e1g1");
        assert!(castle.is_castle() && castle.is_king_castle());
    }

    #[test]
    fn null_move_is_distinguishable() {
        assert!(Move::NULL.is_null());
        let quiet = Move::new(
            Square::from_uci("b1").unwrap(),
            Square::from_uci("c3").unwrap(),
            MoveFlag::Quiet,
        );
        assert!(!quiet.is_null());
    }
}
