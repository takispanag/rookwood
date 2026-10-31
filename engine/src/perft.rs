//! Perft: count leaf nodes of the move tree.
//!
//! Perft is the sharpest tool available for validating a move generator: a
//! single missing en-passant capture, a wrong castling rule or a bad
//! promotion shows up as a number that differs from the published counts.

use crate::movegen::generate_legal;
use crate::moves::Move;
use crate::position::Position;

/// Total leaf nodes at `depth`.
pub fn perft(position: &mut Position, depth: u32) -> u64 {
    if depth == 0 {
        return 1;
    }
    let moves = generate_legal(position);
    if depth == 1 {
        return moves.len() as u64;
    }
    let mut nodes = 0;
    for &mv in moves.iter() {
        let undo = position.make_move(mv);
        nodes += perft(position, depth - 1);
        position.unmake_move(mv, undo);
    }
    nodes
}

/// Per-root-move node counts, the format used to bisect a perft mismatch.
pub fn divide(position: &mut Position, depth: u32) -> Vec<(Move, u64)> {
    let moves = generate_legal(position);
    let mut result = Vec::with_capacity(moves.len());
    for &mv in moves.iter() {
        let undo = position.make_move(mv);
        let nodes = perft(position, depth.saturating_sub(1));
        position.unmake_move(mv, undo);
        result.push((mv, nodes));
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn count(fen: &str, depth: u32) -> u64 {
        let mut position = Position::from_fen(fen).expect("valid fen");
        perft(&mut position, depth)
    }

    #[test]
    fn startpos_shallow() {
        let fen = crate::position::STARTPOS_FEN;
        assert_eq!(count(fen, 1), 20);
        assert_eq!(count(fen, 2), 400);
        assert_eq!(count(fen, 3), 8902);
        assert_eq!(count(fen, 4), 197_281);
    }

    #[test]
    fn kiwipete_shallow() {
        let fen = "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1";
        assert_eq!(count(fen, 1), 48);
        assert_eq!(count(fen, 2), 2039);
        assert_eq!(count(fen, 3), 97_862);
    }

    #[test]
    fn tricky_positions_shallow() {
        assert_eq!(count("8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1", 3), 2812);
        assert_eq!(
            count(
                "r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1",
                3
            ),
            9467
        );
        assert_eq!(
            count(
                "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8",
                3
            ),
            62_379
        );
        assert_eq!(
            count(
                "r4rk1/1pp1qppp/p1np1n2/2b1p1B1/2B1P1b1/P1NP1N2/1PP1QPPP/R4RK1 w - - 0 10",
                3
            ),
            89_890
        );
    }

    #[test]
    fn divide_totals_match_perft() {
        let mut position = Position::startpos();
        let total: u64 = divide(&mut position, 3)
            .iter()
            .map(|(_, nodes)| nodes)
            .sum();
        assert_eq!(total, 8902);
    }
}
