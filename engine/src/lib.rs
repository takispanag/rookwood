//! Rookwood - a chess engine written from scratch.
//!
//! Every part of the engine lives in this crate: the board representation,
//! move generation, the rules, evaluation, search and the UCI protocol.
//!
//! The modules are introduced in dependency order as the engine grows.

pub mod attacks;
pub mod bitboard;
pub mod game;
pub mod movegen;
pub mod moves;
pub mod perft;
pub mod position;
pub mod types;
pub mod zobrist;

pub use game::{DrawReason, Game, GameStatus};
pub use moves::{Move, MoveFlag, MoveList};
pub use position::{CastlingRights, FenError, Position, STARTPOS_FEN};
pub use types::{Color, Piece, PieceType, Square};

/// Engine name reported over UCI and shown in the GUI.
pub const ENGINE_NAME: &str = "Rookwood";
/// Engine version, taken from `Cargo.toml`.
pub const ENGINE_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Version string such as `Rookwood 0.1.0`.
pub fn version_string() -> String {
    format!("{ENGINE_NAME} {ENGINE_VERSION}")
}
