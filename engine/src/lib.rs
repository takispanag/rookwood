//! Rookwood — a chess engine written from scratch.
//!
//! Nothing in this crate depends on another chess library: the board
//! representation, move generation, rules, evaluation, search and the UCI
//! protocol are all implemented here. The only dependency of the whole project
//! is `eframe`/`egui`, and that is used solely by the desktop board in the
//! sibling `rookwood-gui` crate.
//!
//! # Quick start
//!
//! ```
//! use rookwood::{Game, Move};
//!
//! let mut game = Game::new();
//! let mv = game.play_uci("e2e4").expect("e2e4 is legal");
//! assert_eq!(mv.to_uci(), "e2e4");
//! assert_eq!(game.position().side_to_move(), rookwood::Color::Black);
//! ```
//!
//! The engine plays with [`search::search`]:
//!
//! ```
//! use rookwood::{search::search, Game, SearchLimits};
//! use std::sync::Arc;
//! use std::sync::atomic::AtomicBool;
//!
//! let mut game = Game::from_fen("6k1/5ppp/8/8/8/8/5PPP/R5K1 w - - 0 1").unwrap();
//! let result = search(&mut game, SearchLimits::with_depth(3), Arc::new(AtomicBool::new(false)));
//! assert_eq!(result.best_move.unwrap().to_uci(), "a1a8");
//! ```
//!
//! # Layout
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
//! | [`search`] | alpha-beta with quiescence |
//! | [`perft`] | move generator validation |
//! | [`uci`] | the UCI command loop |

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
pub mod uci;
pub mod zobrist;

pub use game::{DrawReason, Game, GameStatus};
pub use moves::{Move, MoveFlag, MoveList};
pub use position::{CastlingRights, FenError, Position, STARTPOS_FEN};
pub use search::{SearchLimits, SearchResult};
pub use types::{Color, Piece, PieceType, Square};

/// Engine name reported over UCI and shown in the GUI.
pub const ENGINE_NAME: &str = "Rookwood";
/// Engine version, taken from `Cargo.toml`.
pub const ENGINE_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Version string such as `Rookwood 0.1.0`.
pub fn version_string() -> String {
    format!("{ENGINE_NAME} {ENGINE_VERSION}")
}
