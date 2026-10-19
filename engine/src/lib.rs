//! Rookwood — a chess engine written from scratch.
//!
//! This crate implements the board representation, attack generation, move
//! legality, FEN and the perft validator directly. It has no dependency on any
//! other chess library.
//!
//! | Module | Responsibility |
//! |---|---|
//! | [`types`] | colours, pieces, squares |
//! | [`bitboard`] | 64-bit square sets |
//! | [`attacks`] | precomputed leaper and sliding attacks |
//! | [`moves`] | 16-bit move encoding and move lists |
//! | [`position`] | board state, FEN, make/unmake |
//! | [`movegen`] | pseudo-legal generation and legality filtering |
//! | [`perft`] | move generator validation |
//! | [`zobrist`] | position keys |

pub mod attacks;
pub mod bitboard;
pub mod movegen;
pub mod moves;
pub mod perft;
pub mod position;
pub mod types;
pub mod zobrist;

pub use moves::{Move, MoveFlag, MoveList};
pub use position::{CastlingRights, FenError, Position, STARTPOS_FEN};
pub use types::{Color, Piece, PieceType, Square};