//! A game in progress: a position plus the history needed for the rules.
//!
//! [`Position`] alone does not know how many times it has occurred, so draw
//! detection by repetition lives here. The search pushes and pops moves on a
//! `Game` so it can see the same history the players do.

use crate::movegen::{generate_captures, generate_legal};
use crate::moves::{Move, MoveList};
use crate::position::{FenError, Position, Undo};
use crate::types::Color;
use std::fmt;

/// Why a game ended in a draw.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DrawReason {
    FiftyMove,
    ThreefoldRepetition,
    InsufficientMaterial,
}

impl DrawReason {
    pub const fn description(self) -> &'static str {
        match self {
            DrawReason::FiftyMove => "draw by the fifty-move rule",
            DrawReason::ThreefoldRepetition => "draw by threefold repetition",
            DrawReason::InsufficientMaterial => "draw by insufficient material",
        }
    }
}

/// The state of a game after the most recent move.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameStatus {
    Ongoing,
    Checkmate,
    Stalemate,
    Draw(DrawReason),
}

impl GameStatus {
    pub const fn is_over(self) -> bool {
        !matches!(self, GameStatus::Ongoing)
    }

    /// Short label suitable for a UI badge.
    pub const fn short(self) -> &'static str {
        match self {
            GameStatus::Ongoing => "in progress",
            GameStatus::Checkmate => "checkmate",
            GameStatus::Stalemate => "stalemate",
            GameStatus::Draw(DrawReason::ThreefoldRepetition) => "draw: repetition",
            GameStatus::Draw(DrawReason::InsufficientMaterial) => "draw: material",
            GameStatus::Draw(DrawReason::FiftyMove) => "draw: fifty moves",
        }
    }
}

impl fmt::Display for GameStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GameStatus::Ongoing => f.write_str("game in progress"),
            GameStatus::Checkmate => {
                // The side to move is the one that has been mated.
                f.write_str("checkmate")
            }
            GameStatus::Stalemate => f.write_str("stalemate"),
            GameStatus::Draw(reason) => f.write_str(reason.description()),
        }
    }
}

/// A position and its move history.
#[derive(Clone, Debug)]
pub struct Game {
    position: Position,
    /// Zobrist key of every position reached, oldest first. The last entry is
    /// always the key of `position`.
    keys: Vec<u64>,
    moves: Vec<Move>,
    undos: Vec<Undo>,
}

impl Default for Game {
    fn default() -> Self {
        Game::new()
    }
}

impl Game {
    /// A fresh game from the standard starting position.
    pub fn new() -> Game {
        Game::from_position(Position::startpos())
    }

    pub fn from_fen(fen: &str) -> Result<Game, FenError> {
        Position::from_fen(fen).map(Game::from_position)
    }

    pub fn from_position(position: Position) -> Game {
        let key = position.key();
        Game {
            position,
            keys: vec![key],
            moves: Vec::new(),
            undos: Vec::new(),
        }
    }

    #[inline]
    pub fn position(&self) -> &Position {
        &self.position
    }

    #[inline]
    pub fn side_to_move(&self) -> Color {
        self.position.side_to_move()
    }

    #[inline]
    pub fn ply(&self) -> usize {
        self.moves.len()
    }

    #[inline]
    pub fn moves(&self) -> &[Move] {
        &self.moves
    }

    pub fn fen(&self) -> String {
        self.position.to_fen()
    }

    pub fn legal_moves(&mut self) -> MoveList {
        generate_legal(&mut self.position)
    }

    pub fn captures(&mut self) -> MoveList {
        generate_captures(&mut self.position)
    }

    /// Play `mv` without checking that it is legal.
    ///
    /// Reserved for callers that just generated the move, or that are stepping
    /// through a trusted game score. User input should go through
    /// [`Game::try_move`].
    pub fn push_move(&mut self, mv: Move) {
        let undo = self.position.make_move(mv);
        self.moves.push(mv);
        self.undos.push(undo);
        self.keys.push(self.position.key());
    }

    /// Play `mv` if it is legal, returning whether it was.
    pub fn try_move(&mut self, mv: Move) -> bool {
        if !self.legal_moves().contains(mv) {
            return false;
        }
        self.push_move(mv);
        true
    }

    /// Play a move given in coordinate notation (`e2e4`, `e7e8q`).
    pub fn play_uci(&mut self, text: &str) -> Option<Move> {
        let mv = self.parse_uci(text)?;
        self.push_move(mv);
        Some(mv)
    }

    /// Resolve coordinate notation against the legal moves of the position.
    pub fn parse_uci(&mut self, text: &str) -> Option<Move> {
        let text = text.trim().to_ascii_lowercase();
        self.legal_moves()
            .iter()
            .copied()
            .find(|mv| mv.to_uci() == text)
    }

    /// Take back the last move.
    pub fn pop_move(&mut self) -> Option<Move> {
        let mv = self.moves.pop()?;
        let undo = self.undos.pop().expect("every move has an undo record");
        self.position.unmake_move(mv, undo);
        self.keys.pop();
        Some(mv)
    }

    pub fn reset(&mut self) {
        *self = Game::new();
    }

    pub fn set_position(&mut self, position: Position) {
        *self = Game::from_position(position);
    }

    /// How often the current position has occurred, counting itself.
    pub fn repetition_count(&self) -> u32 {
        let current = self.position.key();
        let halfmoves = self.position.halfmove_clock() as usize;
        let len = self.keys.len();
        if len < 2 {
            return 1;
        }
        let start = len.saturating_sub(halfmoves + 1);

        let mut count = 1; // the current position
        let mut index = len as isize - 3;
        while index >= start as isize && index >= 0 {
            if self.keys[index as usize] == current {
                count += 1;
            }
            index -= 2; // only positions with the same side to move can repeat
        }
        count
    }

    /// True when the current position has occurred before. The search treats
    /// this as a draw, which is what a repetition *claim* would eventually be.
    pub fn is_repetition(&self) -> bool {
        self.repetition_count() >= 2
    }

    pub fn is_threefold(&self) -> bool {
        self.repetition_count() >= 3
    }

    pub fn is_fifty_move_draw(&self) -> bool {
        self.position.halfmove_clock() >= 100
    }

    pub fn is_insufficient_material(&self) -> bool {
        self.position.insufficient_material()
    }

    /// Checkmate and stalemate take priority: a move that mates on the hundredth
    /// half-move is still mate.
    pub fn status(&mut self) -> GameStatus {
        if self.legal_moves().is_empty() {
            return if self.position.in_check() {
                GameStatus::Checkmate
            } else {
                GameStatus::Stalemate
            };
        }
        if self.is_insufficient_material() {
            return GameStatus::Draw(DrawReason::InsufficientMaterial);
        }
        if self.is_fifty_move_draw() {
            return GameStatus::Draw(DrawReason::FiftyMove);
        }
        if self.is_threefold() {
            return GameStatus::Draw(DrawReason::ThreefoldRepetition);
        }
        GameStatus::Ongoing
    }
}

impl fmt::Display for Game {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{}", self.position)?;
        write!(f, "\nmoves:")?;
        for mv in &self.moves {
            write!(f, " {mv}")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fool_smate_is_detected() {
        let mut game = Game::new();
        for text in ["f2f3", "e7e5", "g2g4", "d8h4"] {
            assert!(game.play_uci(text).is_some(), "could not play {text}");
        }
        assert_eq!(game.status(), GameStatus::Checkmate);
    }

    #[test]
    fn stalemate_is_detected() {
        let mut game = Game::from_fen("7k/5Q2/6K1/8/8/8/8/8 b - - 0 1").expect("valid fen");
        assert_eq!(game.status(), GameStatus::Stalemate);
    }

    #[test]
    fn threefold_repetition_is_counted() {
        let mut game = Game::new();
        // Knights out and back, twice: the starting position appears 3 times.
        for text in [
            "g1f3", "g8f6", "f3g1", "f6g8", "g1f3", "g8f6", "f3g1", "f6g8",
        ] {
            assert!(game.play_uci(text).is_some());
        }
        assert_eq!(game.repetition_count(), 3);
        assert!(game.is_threefold());
        assert_eq!(
            game.status(),
            GameStatus::Draw(DrawReason::ThreefoldRepetition)
        );
    }

    #[test]
    fn fifty_move_rule_uses_the_halfmove_clock() {
        let mut game = Game::from_fen("4k3/8/8/8/8/8/4R3/4K3 w - - 99 80").expect("valid fen");
        assert!(!game.is_fifty_move_draw());
        assert!(game.play_uci("e2e3").is_some());
        assert!(game.is_fifty_move_draw());
        assert_eq!(game.status(), GameStatus::Draw(DrawReason::FiftyMove));
    }

    #[test]
    fn insufficient_material_is_recognised() {
        for fen in [
            "4k3/8/8/8/8/8/8/4K3 w - - 0 1",
            "4k3/8/8/8/8/8/8/4KB2 w - - 0 1",
            "4k3/8/8/8/8/8/8/4KN2 w - - 0 1",
            "2b1k3/8/8/8/8/8/8/4KB2 w - - 0 1",
        ] {
            let game = Game::from_fen(fen).expect("valid fen");
            assert!(
                game.is_insufficient_material(),
                "{fen} should be a dead draw"
            );
        }
        // Two knights can, in principle, be mated against.
        let game = Game::from_fen("4k3/8/8/8/8/8/8/3NKN2 w - - 0 1").expect("valid fen");
        assert!(!game.is_insufficient_material());
        // Opposite-coloured bishops: not forced, but enough material to mate.
        let game = Game::from_fen("2b1k3/8/8/8/8/8/8/2B1K3 w - - 0 1").expect("valid fen");
        assert!(!game.is_insufficient_material());
    }

    #[test]
    fn illegal_input_is_rejected_without_mutating() {
        let mut game = Game::new();
        let fen = game.fen();
        assert!(!game.try_move(Move::NULL));
        assert!(game.play_uci("e2e5").is_none());
        assert_eq!(game.fen(), fen);
        assert_eq!(game.ply(), 0);
    }

    #[test]
    fn unmake_restores_everything() {
        let mut game = Game::new();
        let fen = game.fen();
        for text in ["e2e4", "c7c5", "g1f3", "d7d6", "d2d4", "c5d4", "f3d4"] {
            assert!(game.play_uci(text).is_some());
        }
        while game.pop_move().is_some() {}
        assert_eq!(game.fen(), fen);
        assert_eq!(game.position().key(), Position::startpos().key());
    }
}
