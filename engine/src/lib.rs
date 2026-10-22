//! Rookwood — a chess engine written from scratch.
//!
//! This crate implements the board representation, attack generation, move
//! legality, FEN, the game rules, the evaluation and the search directly. It
//! has no dependency on any other chess library.
//!
//! | Module | Responsibility |
//! |---|---|
//! | [`types`] | colours, pieces, squares |
//! | [`bitboard`] | 64-bit square sets |
//! | [`attacks`] | precomputed leaper and sliding attacks |
//! | [`moves`] | 16-bit move encoding and move lists |
//! | [`position`] | board state, FEN, make/unmake |
//! | [`movegen`] | pseudo-legal generation and legality filtering |
//! | [`game`] | move history and draw rules |
//! | [`eval`] | material and piece-square evaluation |
//! | [`search`] | alpha-beta and quiescence |
//! | [`perft`] | move generator validation |
//! | [`zobrist`] | position keys |

pub mod attacks;
pub mod bitboard;
pub mod eval;
pub mod game;
pub mod movegen;
pub mod moves;
pub mod perft;
pub mod position;
pub mod search;
pub mod types;
pub mod zobrist;

pub use game::{DrawReason, Game, GameStatus};
pub use moves::{Move, MoveFlag, MoveList};
pub use position::{CastlingRights, FenError, Position, STARTPOS_FEN};
pub use search::{SearchLimits, SearchResult};
pub use types::{Color, Piece, PieceType, Square};