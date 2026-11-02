//! Alpha-beta search.
//!
//! The shape is deliberately plain: fixed-depth negamax with alpha-beta
//! pruning, a capture-only quiescence search, and MVV-LVA move ordering. What
//! it lacks in sophistication it makes up for in being easy to reason about,
//! and it is the foundation the later search work builds on.

use crate::eval::{evaluate, piece_value};
use crate::game::Game;
use crate::moves::Move;
use crate::position::Position;
use crate::types::Color;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// Deepest line the search will follow. Keeps the per-ply tables bounded.
pub const MAX_PLY: usize = 64;
/// Score reported for a forced mate, offset by the distance to it.
pub const MATE_SCORE: i32 = 30_000;
/// Anything within this distance of [`MATE_SCORE`] is a mate score.
pub const MATE_THRESHOLD: i32 = MATE_SCORE - MAX_PLY as i32;
pub const INFINITY: i32 = 32_000;

/// How much work to do.
#[derive(Clone, Copy, Debug)]
pub struct SearchLimits {
    /// Maximum search depth in plies.
    pub depth: u8,
    /// Wall-clock budget. The search stops when it is exhausted.
    pub movetime: Option<Duration>,
    /// Hard node budget, checked alongside the clock.
    pub nodes: Option<u64>,
}

impl Default for SearchLimits {
    fn default() -> Self {
        SearchLimits {
            depth: 5,
            movetime: None,
            nodes: None,
        }
    }
}

impl SearchLimits {
    pub const fn with_depth(depth: u8) -> SearchLimits {
        SearchLimits {
            depth,
            movetime: None,
            nodes: None,
        }
    }

    pub const fn with_movetime(millis: u64) -> SearchLimits {
        SearchLimits {
            depth: 64,
            movetime: Some(Duration::from_millis(millis)),
            nodes: None,
        }
    }
}

/// What the search found.
#[derive(Clone, Debug)]
pub struct SearchResult {
    pub best_move: Option<Move>,
    /// Score from the point of view of the side that was to move.
    pub score: i32,
    pub depth: u8,
    pub nodes: u64,
    pub pv: Vec<Move>,
    pub elapsed: Duration,
    /// True when the search ran out of time or was stopped early.
    pub interrupted: bool,
}

impl SearchResult {
    /// `Some(n)` when the score is a forced mate for the side to move (`n > 0`)
    /// or against it (`n < 0`), in moves rather than plies.
    pub fn mate_in(&self) -> Option<i32> {
        if self.score.abs() < MATE_THRESHOLD {
            return None;
        }
        let plies = MATE_SCORE - self.score.abs();
        let moves = (plies + 1) / 2;
        Some(if self.score > 0 { moves } else { -moves })
    }

    pub fn nodes_per_second(&self) -> u64 {
        let millis = self.elapsed.as_millis().max(1) as u64;
        self.nodes.saturating_mul(1000) / millis
    }
}

/// Search a position.
///
/// The caller owns the clock and the cancellation flag so the same entry point
/// works for the UCI loop, the GUI worker thread and the tests.
pub fn search(game: &mut Game, limits: SearchLimits, stop: Arc<AtomicBool>) -> SearchResult {
    let start = Instant::now();

    // Leave a little room so the move is printed before the clock runs out.
    let deadline = limits
        .movetime
        .map(|budget| start + budget.saturating_sub(Duration::from_millis(20)));

    let mut searcher = Searcher {
        nodes: 0,
        stop,
        deadline,
        node_limit: limits.nodes,
        interrupted: false,
        pv_length: [0; MAX_PLY + 1],
        pv_table: [[Move::NULL; MAX_PLY + 1]; MAX_PLY + 1],
    };

    let root_moves = game.legal_moves();
    if root_moves.is_empty() {
        return SearchResult {
            best_move: None,
            score: if game.position().in_check() {
                -MATE_SCORE
            } else {
                0
            },
            depth: 0,
            nodes: 0,
            pv: Vec::new(),
            elapsed: start.elapsed(),
            interrupted: false,
        };
    }

    let depth = limits.depth.max(1) as i32;
    let score = searcher.negamax(game, depth, -INFINITY, INFINITY, 0);

    let pv_len = searcher.pv_length[0];
    let pv: Vec<Move> = searcher.pv_table[0][..pv_len.min(MAX_PLY)].to_vec();
    let best_move = pv.first().copied().or_else(|| {
        if searcher.interrupted {
            root_moves.first().copied()
        } else {
            None
        }
    });

    SearchResult {
        best_move,
        score,
        depth: limits.depth,
        nodes: searcher.nodes,
        pv,
        elapsed: start.elapsed(),
        interrupted: searcher.interrupted,
    }
}

struct Searcher {
    nodes: u64,
    stop: Arc<AtomicBool>,
    deadline: Option<Instant>,
    node_limit: Option<u64>,
    interrupted: bool,
    /// `pv_length[ply]` is the index one past the last move of the PV at `ply`.
    pv_length: [usize; MAX_PLY + 1],
    pv_table: [[Move; MAX_PLY + 1]; MAX_PLY + 1],
}

impl Searcher {
    /// Check the clock and the stop flag every 2048 nodes; `Instant::now` is
    /// not free and the search does not need millisecond precision.
    #[inline]
    fn poll_stop(&mut self) -> bool {
        if self.interrupted {
            return true;
        }
        if self.nodes & 2047 == 0 {
            if self.stop.load(Ordering::Relaxed) {
                self.interrupted = true;
                return true;
            }
            if let Some(deadline) = self.deadline {
                if Instant::now() >= deadline {
                    self.interrupted = true;
                    return true;
                }
            }
            if let Some(limit) = self.node_limit {
                if self.nodes >= limit {
                    self.interrupted = true;
                    return true;
                }
            }
        }
        false
    }

    fn negamax(
        &mut self,
        game: &mut Game,
        depth: i32,
        mut alpha: i32,
        beta: i32,
        ply: usize,
    ) -> i32 {
        self.pv_length[ply] = ply;
        if self.poll_stop() {
            return 0;
        }
        self.nodes += 1;

        if ply > 0 {
            if game.is_repetition() || game.is_fifty_move_draw() || game.is_insufficient_material()
            {
                return 0;
            }
            if ply >= MAX_PLY - 1 {
                return evaluate(game.position());
            }
        }

        if depth <= 0 {
            return self.quiescence(game, alpha, beta, ply);
        }

        let mut moves = game.legal_moves();
        if moves.is_empty() {
            return if game.position().in_check() {
                -MATE_SCORE + ply as i32
            } else {
                0
            };
        }

        let count = moves.len();
        let mut scores = [0i32; 256];
        for (index, &mv) in moves.iter().enumerate() {
            scores[index] = move_order_score(game.position(), mv);
        }

        let mut best = -INFINITY;
        for index in 0..count {
            let pick = best_index(&scores, index, count);
            if pick != index {
                moves.swap(index, pick);
                scores.swap(index, pick);
            }
            let mv = moves[index];

            game.push_move(mv);
            let score = -self.negamax(game, depth - 1, -beta, -alpha, ply + 1);
            game.pop_move();

            if self.interrupted {
                return 0;
            }

            if score > best {
                best = score;
                if score > alpha {
                    alpha = score;

                    // Extend the principal variation through this move.
                    let child_len = self.pv_length[ply + 1];
                    let (upper, lower) = self.pv_table.split_at_mut(ply + 1);
                    upper[ply][ply] = mv;
                    upper[ply][ply + 1..child_len].copy_from_slice(&lower[0][ply + 1..child_len]);
                    self.pv_length[ply] = child_len;

                    if score >= beta {
                        break;
                    }
                }
            }
        }

        best
    }

    fn quiescence(&mut self, game: &mut Game, mut alpha: i32, beta: i32, ply: usize) -> i32 {
        if self.poll_stop() {
            return 0;
        }
        self.nodes += 1;

        if ply >= MAX_PLY - 1 {
            return evaluate(game.position());
        }

        // Stand pat: the side to move is not obliged to capture.
        let stand_pat = evaluate(game.position());
        if stand_pat >= beta {
            return beta;
        }
        if stand_pat > alpha {
            alpha = stand_pat;
        }

        let mut moves = game.captures();
        if moves.is_empty() {
            return alpha;
        }

        let count = moves.len();
        let mut scores = [0i32; 256];
        for (index, &mv) in moves.iter().enumerate() {
            scores[index] = move_order_score(game.position(), mv);
        }

        for index in 0..count {
            let pick = best_index(&scores, index, count);
            if pick != index {
                moves.swap(index, pick);
                scores.swap(index, pick);
            }
            let mv = moves[index];

            game.push_move(mv);
            let score = -self.quiescence(game, -beta, -alpha, ply + 1);
            game.pop_move();

            if self.interrupted {
                return 0;
            }
            if score >= beta {
                return beta;
            }
            if score > alpha {
                alpha = score;
            }
        }

        alpha
    }
}

/// Index of the highest score in `scores[from..to]`.
#[inline]
fn best_index(scores: &[i32; 256], from: usize, to: usize) -> usize {
    let mut best = from;
    for index in from + 1..to {
        if scores[index] > scores[best] {
            best = index;
        }
    }
    best
}

/// Most-valuable-victim / least-valuable-attacker, plus promotions.
#[inline]
fn move_order_score(position: &Position, mv: Move) -> i32 {
    if let Some(kind) = mv.promotion() {
        return 900_000 + piece_value(kind);
    }
    if mv.is_en_passant() {
        let attacker = position
            .piece_on(mv.from())
            .map_or(0, |piece| piece_value(piece.kind));
        return 800_000 + 10 * crate::eval::PAWN_VALUE - attacker;
    }
    match (position.piece_on(mv.to()), position.piece_on(mv.from())) {
        (Some(victim), Some(attacker)) => {
            800_000 + 10 * piece_value(victim.kind) - piece_value(attacker.kind)
        }
        _ => 0,
    }
}

/// Pick the move a fixed-depth search prefers, without any clock. Handy in
/// tests and in the GUI's "think now" button.
pub fn best_move(game: &mut Game, depth: u8) -> Option<Move> {
    search(
        game,
        SearchLimits::with_depth(depth),
        Arc::new(AtomicBool::new(false)),
    )
    .best_move
}

/// Convenience for tests: the score of a search at a fixed depth.
pub fn score_at_depth(game: &mut Game, depth: u8) -> i32 {
    search(
        game,
        SearchLimits::with_depth(depth),
        Arc::new(AtomicBool::new(false)),
    )
    .score
}

/// Score to the side to move, for a caller that knows which colour that was.
pub fn relative_to(score: i32, side: Color) -> i32 {
    match side {
        Color::White => score,
        Color::Black => -score,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn find_move(fen: &str, depth: u8) -> String {
        let mut game = Game::from_fen(fen).expect("valid fen");
        let result = search(
            &mut game,
            SearchLimits::with_depth(depth),
            Arc::new(AtomicBool::new(false)),
        );
        result
            .best_move
            .expect("a legal position has a move")
            .to_uci()
    }

    #[test]
    fn finds_mate_in_one() {
        // Ra8# is the classic back-rank mate: the pawns on f7/g7/h7 seal the
        // king in and nothing can interpose on the eighth rank.
        let mv = find_move("6k1/5ppp/8/8/8/8/5PPP/R5K1 w - - 0 1", 3);
        assert_eq!(mv, "a1a8");
    }

    #[test]
    fn takes_a_free_queen() {
        let mv = find_move("4k3/8/8/3q4/4P3/8/8/4K3 w - - 0 1", 3);
        assert_eq!(mv, "e4d5");
    }

    #[test]
    fn avoids_stalemate_when_winning() {
        // Queen and king against a bare king: any sane search keeps the win.
        let mut game = Game::from_fen("7k/8/8/8/8/8/8/K6Q w - - 0 1").expect("valid fen");
        let result = search(
            &mut game,
            SearchLimits::with_depth(4),
            Arc::new(AtomicBool::new(false)),
        );
        assert!(
            result.score > 500,
            "expected a winning score, got {}",
            result.score
        );
    }

    #[test]
    fn detects_mate_scores() {
        // Black is mated, so the score is a loss whatever the depth.
        let mut game =
            Game::from_fen("rnb1kbnr/pppp1ppp/8/4p3/6Pq/5P2/PPPPP2P/RNBQKBNR w KQkq - 1 3")
                .unwrap();
        let result = search(
            &mut game,
            SearchLimits::with_depth(3),
            Arc::new(AtomicBool::new(false)),
        );
        assert_eq!(result.best_move, None);
        assert!(result.score <= -MATE_THRESHOLD);
    }

    #[test]
    fn respects_the_node_limit() {
        let mut game = Game::new();
        let limits = SearchLimits {
            depth: 60,
            movetime: None,
            nodes: Some(2_000),
        };
        let result = search(&mut game, limits, Arc::new(AtomicBool::new(false)));
        assert!(result.interrupted);
        assert!(result.nodes >= 2_000 && result.nodes < 4_096);
        assert!(
            result.best_move.is_some(),
            "an interrupted search should still return a move"
        );
    }

    #[test]
    fn respects_the_stop_flag() {
        let stop = Arc::new(AtomicBool::new(true));
        let mut game = Game::new();
        let result = search(&mut game, SearchLimits::with_depth(20), stop);
        assert!(result.interrupted);
        assert!(result.best_move.is_some());
    }

    #[test]
    fn pv_is_a_playable_line() {
        let mut game = Game::new();
        let result = search(
            &mut game,
            SearchLimits::with_depth(3),
            Arc::new(AtomicBool::new(false)),
        );
        assert!(!result.pv.is_empty());
        for &mv in &result.pv {
            assert!(game.try_move(mv), "PV move {mv} is not legal");
        }
    }
}
