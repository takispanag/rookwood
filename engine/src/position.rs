//! Board state, FEN input/output, and making/unmaking moves.
//!
//! [`Position`] keeps three views of the same board in sync:
//!
//! * a bitboard per piece type (`pieces`),
//! * a bitboard per colour (`colors`),
//! * a mailbox (`board`) for "what is on this square" queries.
//!
//! Duplication costs a little memory but makes move generation and evaluation
//! straightforward, and `debug_assert!`s in the tests keep the views honest.

use crate::attacks;
use crate::bitboard::Bitboard;
use crate::moves::Move;
use crate::types::{Color, FILE_NAMES, Piece, PieceType, Square};
use crate::zobrist::ZOBRIST;
use std::fmt;

pub const STARTPOS_FEN: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

/// Which castling moves are still available.
#[derive(Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct CastlingRights(u8);

impl CastlingRights {
    pub const NONE: CastlingRights = CastlingRights(0);
    pub const WHITE_KING_SIDE: CastlingRights = CastlingRights(1);
    pub const WHITE_QUEEN_SIDE: CastlingRights = CastlingRights(2);
    pub const BLACK_KING_SIDE: CastlingRights = CastlingRights(4);
    pub const BLACK_QUEEN_SIDE: CastlingRights = CastlingRights(8);
    pub const ALL: CastlingRights = CastlingRights(15);

    #[inline]
    pub const fn king_side(color: Color) -> CastlingRights {
        match color {
            Color::White => CastlingRights::WHITE_KING_SIDE,
            Color::Black => CastlingRights::BLACK_KING_SIDE,
        }
    }

    #[inline]
    pub const fn queen_side(color: Color) -> CastlingRights {
        match color {
            Color::White => CastlingRights::WHITE_QUEEN_SIDE,
            Color::Black => CastlingRights::BLACK_QUEEN_SIDE,
        }
    }

    /// Both rights belonging to one colour.
    #[inline]
    pub const fn both(color: Color) -> CastlingRights {
        CastlingRights(CastlingRights::king_side(color).0 | CastlingRights::queen_side(color).0)
    }

    #[inline]
    pub const fn bits(self) -> u8 {
        self.0
    }

    #[inline]
    pub const fn from_bits(bits: u8) -> CastlingRights {
        CastlingRights(bits & 0x0f)
    }

    #[inline]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    #[inline]
    pub const fn has(self, right: CastlingRights) -> bool {
        self.0 & right.0 != 0
    }

    #[inline]
    pub fn add(&mut self, right: CastlingRights) {
        self.0 |= right.0;
    }

    #[inline]
    pub fn remove(&mut self, right: CastlingRights) {
        self.0 &= !right.0;
    }

    /// The castling right, if any, that lives on `square` (a corner square).
    #[inline]
    pub const fn right_for_corner(square: Square) -> CastlingRights {
        match square.index() {
            0 => CastlingRights::WHITE_QUEEN_SIDE,
            7 => CastlingRights::WHITE_KING_SIDE,
            56 => CastlingRights::BLACK_QUEEN_SIDE,
            63 => CastlingRights::BLACK_KING_SIDE,
            _ => CastlingRights::NONE,
        }
    }

    pub fn from_fen(field: &str) -> Result<CastlingRights, FenError> {
        if field == "-" {
            return Ok(CastlingRights::NONE);
        }
        let mut rights = CastlingRights::NONE;
        for ch in field.chars() {
            let right = match ch {
                'K' => CastlingRights::WHITE_KING_SIDE,
                'Q' => CastlingRights::WHITE_QUEEN_SIDE,
                'k' => CastlingRights::BLACK_KING_SIDE,
                'q' => CastlingRights::BLACK_QUEEN_SIDE,
                _ => return Err(FenError::Castling(field.to_string())),
            };
            if rights.has(right) {
                return Err(FenError::Castling(field.to_string()));
            }
            rights.add(right);
        }
        Ok(rights)
    }

    pub fn to_fen(self) -> String {
        if self.is_empty() {
            return "-".to_string();
        }
        let mut text = String::with_capacity(4);
        for (right, letter) in [
            (CastlingRights::WHITE_KING_SIDE, 'K'),
            (CastlingRights::WHITE_QUEEN_SIDE, 'Q'),
            (CastlingRights::BLACK_KING_SIDE, 'k'),
            (CastlingRights::BLACK_QUEEN_SIDE, 'q'),
        ] {
            if self.has(right) {
                text.push(letter);
            }
        }
        text
    }
}

impl fmt::Debug for CastlingRights {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CastlingRights({})", self.to_fen())
    }
}

/// Everything needed to undo a move.
///
/// The key is stored verbatim instead of being "un-XOR-ed"; copying eight
/// bytes is cheaper than replaying the hashing logic backwards, and it makes
/// unmake impossible to get subtly wrong.
#[derive(Clone, Copy, Debug)]
pub struct Undo {
    captured: Option<Piece>,
    castling: CastlingRights,
    ep: Option<Square>,
    halfmove: u16,
    key: u64,
}

/// A complete chess position.
#[derive(Clone)]
pub struct Position {
    pieces: [Bitboard; 6],
    colors: [Bitboard; 2],
    board: [Option<Piece>; 64],
    side: Color,
    castling: CastlingRights,
    ep: Option<Square>,
    halfmove: u16,
    fullmove: u32,
    key: u64,
}

impl Position {
    /// An empty board with no kings. Only useful for building positions by hand.
    pub fn empty() -> Position {
        Position {
            pieces: [Bitboard::EMPTY; 6],
            colors: [Bitboard::EMPTY; 2],
            board: [None; 64],
            side: Color::White,
            castling: CastlingRights::NONE,
            ep: None,
            halfmove: 0,
            fullmove: 1,
            key: 0,
        }
    }

    pub fn startpos() -> Position {
        Position::from_fen(STARTPOS_FEN).expect("the starting position is valid")
    }

    // ---------------------------------------------------------------- queries

    #[inline]
    pub fn piece_on(&self, square: Square) -> Option<Piece> {
        self.board[square.index()]
    }

    #[inline]
    pub fn pieces(&self, kind: PieceType) -> Bitboard {
        self.pieces[kind.index()]
    }

    #[inline]
    pub fn color_occupancy(&self, color: Color) -> Bitboard {
        self.colors[color.index()]
    }

    #[inline]
    pub fn occupancy(&self) -> Bitboard {
        self.colors[0] | self.colors[1]
    }

    #[inline]
    pub fn side_to_move(&self) -> Color {
        self.side
    }

    #[inline]
    pub fn castling(&self) -> CastlingRights {
        self.castling
    }

    #[inline]
    pub fn ep_square(&self) -> Option<Square> {
        self.ep
    }

    #[inline]
    pub fn halfmove_clock(&self) -> u16 {
        self.halfmove
    }

    #[inline]
    pub fn fullmove_number(&self) -> u32 {
        self.fullmove
    }

    /// The Zobrist key of this position. Equal keys mean equal positions
    /// (treating a capturable en-passant square as part of the position).
    #[inline]
    pub fn key(&self) -> u64 {
        self.key
    }

    #[inline]
    pub fn king_square(&self, color: Color) -> Square {
        (self.pieces[PieceType::King.index()] & self.colors[color.index()])
            .lsb()
            .expect("position must contain a king for each side")
    }

    /// Is `square` attacked by any piece of `by`?
    #[inline]
    pub fn is_attacked(&self, square: Square, by: Color) -> bool {
        let theirs = self.colors[by.index()];

        // Pawns of `by` attacking `square` stand where a pawn of the other
        // colour would attack it from, hence the flipped colour.
        if attacks::pawn_attacks(by.opposite(), square)
            & self.pieces[PieceType::Pawn.index()]
            & theirs
            != Bitboard::EMPTY
        {
            return true;
        }
        if attacks::knight_attacks(square) & self.pieces[PieceType::Knight.index()] & theirs
            != Bitboard::EMPTY
        {
            return true;
        }
        if attacks::king_attacks(square) & self.pieces[PieceType::King.index()] & theirs
            != Bitboard::EMPTY
        {
            return true;
        }

        let occupancy = self.occupancy();
        let queens = self.pieces[PieceType::Queen.index()];
        if attacks::bishop_attacks(square, occupancy)
            & (self.pieces[PieceType::Bishop.index()] | queens)
            & theirs
            != Bitboard::EMPTY
        {
            return true;
        }
        if attacks::rook_attacks(square, occupancy)
            & (self.pieces[PieceType::Rook.index()] | queens)
            & theirs
            != Bitboard::EMPTY
        {
            return true;
        }
        false
    }

    /// Is the side to move in check?
    #[inline]
    pub fn in_check(&self) -> bool {
        self.is_attacked(self.king_square(self.side), self.side.opposite())
    }

    /// True when neither side can possibly deliver mate any more.
    pub fn insufficient_material(&self) -> bool {
        if self.pieces[PieceType::Pawn.index()] != Bitboard::EMPTY
            || self.pieces[PieceType::Rook.index()] != Bitboard::EMPTY
            || self.pieces[PieceType::Queen.index()] != Bitboard::EMPTY
        {
            return false;
        }

        let knights = self.pieces[PieceType::Knight.index()];
        let bishops = self.pieces[PieceType::Bishop.index()];
        let minor_count = knights.count() + bishops.count();

        if minor_count <= 1 {
            return true;
        }
        // Any number of bishops all living on one colour complex cannot mate.
        if knights == Bitboard::EMPTY {
            let light = bishops & LIGHT_SQUARES;
            return light == Bitboard::EMPTY || light == bishops;
        }
        false
    }

    /// A capturable en-passant square contributes to the key, otherwise the
    /// fifty-move rule and repetition detection would over-count positions
    /// that merely look different.
    #[inline]
    fn ep_key(&self) -> u64 {
        match self.ep {
            Some(square) if self.ep_is_capturable(square) => {
                ZOBRIST.ep_file[square.file() as usize]
            }
            _ => 0,
        }
    }

    fn ep_is_capturable(&self, square: Square) -> bool {
        attacks::pawn_attacks(self.side.opposite(), square)
            & self.pieces[PieceType::Pawn.index()]
            & self.colors[self.side.index()]
            != Bitboard::EMPTY
    }

    /// Recompute the key from scratch. The search relies on incremental
    /// updates matching this exactly; tests assert it on every move.
    pub fn recompute_key(&self) -> u64 {
        let mut key = 0;
        for square in Bitboard::FULL {
            if let Some(piece) = self.board[square.index()] {
                key ^= ZOBRIST.piece(piece.color, piece.kind, square);
            }
        }
        key ^= ZOBRIST.castling[self.castling.bits() as usize];
        key ^= self.ep_key();
        if self.side == Color::Black {
            key ^= ZOBRIST.side;
        }
        key
    }

    // ------------------------------------------------------- state mutation

    #[inline]
    fn add_piece(&mut self, square: Square, piece: Piece) {
        let bit = Bitboard::from_square(square);
        self.pieces[piece.kind.index()] |= bit;
        self.colors[piece.color.index()] |= bit;
        self.board[square.index()] = Some(piece);
        self.key ^= ZOBRIST.piece(piece.color, piece.kind, square);
    }

    #[inline]
    fn remove_piece(&mut self, square: Square) {
        if let Some(piece) = self.board[square.index()] {
            let bit = Bitboard::from_square(square);
            self.pieces[piece.kind.index()] ^= bit;
            self.colors[piece.color.index()] ^= bit;
            self.board[square.index()] = None;
            self.key ^= ZOBRIST.piece(piece.color, piece.kind, square);
        }
    }

    /// Apply `mv` and return the information needed to take it back.
    ///
    /// The move must be pseudo-legal; legality (not leaving your own king in
    /// check) is the caller's responsibility.
    pub fn make_move(&mut self, mv: Move) -> Undo {
        let from = mv.from();
        let to = mv.to();
        let moving = self.board[from.index()].expect("make_move: no piece on origin square");
        let us = moving.color;
        let them = us.opposite();

        let previous = Undo {
            captured: None,
            castling: self.castling,
            ep: self.ep,
            halfmove: self.halfmove,
            key: self.key,
        };

        // Retire the contributions that are about to change.
        self.key ^= self.ep_key();
        self.key ^= ZOBRIST.castling[self.castling.bits() as usize];

        let captured = if mv.is_en_passant() {
            let captured_square = Square::new(to.file(), from.rank());
            let piece = self.board[captured_square.index()];
            self.remove_piece(captured_square);
            piece
        } else {
            let piece = self.board[to.index()];
            if piece.is_some() {
                self.remove_piece(to);
            }
            piece
        };

        self.remove_piece(from);
        let landing = match mv.promotion() {
            Some(kind) => Piece::new(us, kind),
            None => moving,
        };
        self.add_piece(to, landing);

        if mv.is_castle() {
            let rank = from.rank();
            let (rook_from_file, rook_to_file) = if mv.is_king_castle() { (7, 5) } else { (0, 3) };
            self.remove_piece(Square::new(rook_from_file, rank));
            self.add_piece(
                Square::new(rook_to_file, rank),
                Piece::new(us, PieceType::Rook),
            );
        }

        // Castling rights: a king move ends both rights, a rook move or a
        // rook capture ends the right attached to that corner.
        if moving.kind == PieceType::King {
            self.castling.remove(CastlingRights::both(us));
        }
        self.castling.remove(CastlingRights::right_for_corner(from));
        self.castling.remove(CastlingRights::right_for_corner(to));

        self.ep = if mv.is_double_push() {
            Some(Square::new(
                from.file(),
                (from.rank() as i8 + us.forward()) as u8,
            ))
        } else {
            None
        };

        if moving.kind == PieceType::Pawn || captured.is_some() {
            self.halfmove = 0;
        } else {
            self.halfmove = self.halfmove.saturating_add(1);
        }
        if us == Color::Black {
            self.fullmove += 1;
        }

        self.side = them;
        self.key ^= ZOBRIST.side;
        self.key ^= ZOBRIST.castling[self.castling.bits() as usize];
        self.key ^= self.ep_key();

        Undo {
            captured,
            ..previous
        }
    }

    /// Take back the most recent [`Position::make_move`].
    pub fn unmake_move(&mut self, mv: Move, undo: Undo) {
        let from = mv.from();
        let to = mv.to();

        self.side = self.side.opposite();
        if self.side == Color::Black {
            self.fullmove -= 1;
        }

        let landing = self.board[to.index()].expect("unmake_move: no piece on target square");
        self.remove_piece(to);
        let original = if mv.is_promotion() {
            Piece::new(self.side, PieceType::Pawn)
        } else {
            landing
        };
        self.add_piece(from, original);

        if mv.is_castle() {
            let rank = from.rank();
            let (rook_from_file, rook_to_file) = if mv.is_king_castle() { (7, 5) } else { (0, 3) };
            self.remove_piece(Square::new(rook_to_file, rank));
            self.add_piece(
                Square::new(rook_from_file, rank),
                Piece::new(self.side, PieceType::Rook),
            );
        }

        if let Some(piece) = undo.captured {
            let square = if mv.is_en_passant() {
                Square::new(to.file(), from.rank())
            } else {
                to
            };
            self.add_piece(square, piece);
        }

        self.castling = undo.castling;
        self.ep = undo.ep;
        self.halfmove = undo.halfmove;
        self.key = undo.key;
    }

    // ------------------------------------------------------------- FEN

    pub fn from_fen(fen: &str) -> Result<Position, FenError> {
        let fields: Vec<&str> = fen.split_whitespace().collect();
        if fields.len() < 4 {
            return Err(FenError::FieldCount(fields.len()));
        }

        let mut position = Position::empty();

        let ranks: Vec<&str> = fields[0].split('/').collect();
        if ranks.len() != 8 {
            return Err(FenError::RankCount(ranks.len()));
        }
        for (index, rank_text) in ranks.iter().enumerate() {
            let rank = 7 - index as u8;
            let mut file: u8 = 0;
            for ch in rank_text.chars() {
                if let Some(digit) = ch.to_digit(10) {
                    if digit == 0 {
                        return Err(FenError::RankWidth {
                            rank: rank as usize + 1,
                            width: 0,
                        });
                    }
                    file += digit as u8;
                } else {
                    let piece = Piece::from_char(ch).ok_or(FenError::Piece(ch))?;
                    if file >= 8 {
                        return Err(FenError::RankWidth {
                            rank: rank as usize + 1,
                            width: file as usize + 1,
                        });
                    }
                    position.add_piece(Square::new(file, rank), piece);
                    file += 1;
                }
                if file > 8 {
                    return Err(FenError::RankWidth {
                        rank: rank as usize + 1,
                        width: file as usize,
                    });
                }
            }
            if file != 8 {
                return Err(FenError::RankWidth {
                    rank: rank as usize + 1,
                    width: file as usize,
                });
            }
        }

        for color in Color::ALL {
            let kings = position.pieces[PieceType::King.index()] & position.colors[color.index()];
            match kings.count() {
                0 => return Err(FenError::King(color, "missing")),
                1 => {}
                _ => return Err(FenError::King(color, "duplicated")),
            }
        }

        position.side = match fields[1] {
            "w" => Color::White,
            "b" => Color::Black,
            other => return Err(FenError::SideToMove(other.to_string())),
        };

        position.castling = CastlingRights::from_fen(fields[2])?;

        position.ep = match fields[3] {
            "-" => None,
            square => {
                let square = Square::from_uci(square)
                    .ok_or_else(|| FenError::EnPassant(fields[3].to_string()))?;
                if square.rank() != 2 && square.rank() != 5 {
                    return Err(FenError::EnPassant(fields[3].to_string()));
                }
                Some(square)
            }
        };

        if let Some(field) = fields.get(4) {
            position.halfmove = field
                .parse::<u16>()
                .map_err(|_| FenError::Clock(field.to_string()))?;
        }
        if let Some(field) = fields.get(5) {
            let moves = field
                .parse::<u32>()
                .map_err(|_| FenError::Clock(field.to_string()))?;
            if moves == 0 {
                return Err(FenError::Clock(field.to_string()));
            }
            position.fullmove = moves;
        }

        position.key = position.recompute_key();
        Ok(position)
    }

    pub fn to_fen(&self) -> String {
        let mut fen = String::with_capacity(90);
        for rank in (0..8).rev() {
            let mut empty = 0;
            for file in 0..8 {
                match self.board[Square::new(file, rank).index()] {
                    Some(piece) => {
                        if empty > 0 {
                            fen.push(char::from_digit(empty, 10).unwrap());
                            empty = 0;
                        }
                        fen.push(piece.to_char());
                    }
                    None => empty += 1,
                }
            }
            if empty > 0 {
                fen.push(char::from_digit(empty, 10).unwrap());
            }
            if rank > 0 {
                fen.push('/');
            }
        }
        fen.push(' ');
        fen.push(if self.side == Color::White { 'w' } else { 'b' });
        fen.push(' ');
        fen.push_str(&self.castling.to_fen());
        fen.push(' ');
        match self.ep {
            Some(square) => fen.push_str(&square.to_uci()),
            None => fen.push('-'),
        }
        fen.push_str(&format!(" {} {}", self.halfmove, self.fullmove));
        fen
    }
}

const LIGHT_SQUARES: Bitboard = Bitboard(0x55aa_55aa_55aa_55aa);

/// Errors returned by [`Position::from_fen`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FenError {
    FieldCount(usize),
    RankCount(usize),
    RankWidth { rank: usize, width: usize },
    Piece(char),
    King(Color, &'static str),
    SideToMove(String),
    Castling(String),
    EnPassant(String),
    Clock(String),
}

impl fmt::Display for FenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FenError::FieldCount(count) => {
                write!(f, "expected at least 4 FEN fields, found {count}")
            }
            FenError::RankCount(count) => write!(f, "expected 8 ranks, found {count}"),
            FenError::RankWidth { rank, width } => {
                write!(f, "rank {rank} describes {width} squares instead of 8")
            }
            FenError::Piece(ch) => write!(f, "unrecognised piece letter {ch:?}"),
            FenError::King(color, problem) => write!(f, "king for {color} is {problem}"),
            FenError::SideToMove(field) => write!(f, "invalid side to move {field:?}"),
            FenError::Castling(field) => write!(f, "invalid castling field {field:?}"),
            FenError::EnPassant(field) => write!(f, "invalid en-passant square {field:?}"),
            FenError::Clock(field) => write!(f, "invalid move counter {field:?}"),
        }
    }
}

impl std::error::Error for FenError {}

/// Render the board with coordinates, used by the `d` UCI command and tests.
impl fmt::Display for Position {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for rank in (0..8).rev() {
            write!(f, "{} ", rank + 1)?;
            for file in 0..8 {
                match self.board[Square::new(file, rank).index()] {
                    Some(piece) => write!(f, "{} ", piece.to_char())?,
                    None => write!(f, ". ")?,
                }
            }
            if rank == 7 {
                write!(f, "  side: {}", self.side)?;
            }
            writeln!(f)?;
        }
        write!(f, "  ")?;
        for file in FILE_NAMES {
            write!(f, "{file} ")?;
        }
        write!(f, "\n  castling: {}  ep: ", self.castling.to_fen())?;
        match self.ep {
            Some(square) => write!(f, "{square}")?,
            None => write!(f, "-")?,
        }
        write!(
            f,
            "  halfmove: {}  fullmove: {}",
            self.halfmove, self.fullmove
        )
    }
}

impl fmt::Debug for Position {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Position")
            .field("fen", &self.to_fen())
            .field("key", &format_args!("{:#018x}", self.key))
            .finish()
    }
}
