//! Move generation.
//!
//! Generation is split in two: a fast pseudo-legal pass that ignores whether
//! the mover's king ends up in check, and a legality filter that makes and
//! unmakes each candidate. The filter costs a little speed but removes an
//! entire class of pin and discovered-check bugs, which is the right trade for
//! a hand-written generator that has to be trusted.
//!
//! Castling is the exception: its geometric preconditions (rights, empty
//! squares, king not walking through check) are checked during generation,
//! because they cannot be expressed as "make the move and test".

use crate::attacks;
use crate::bitboard::Bitboard;
use crate::moves::{Move, MoveFlag, MoveList};
use crate::position::{CastlingRights, Position};
use crate::types::{Color, Piece, PieceType, Square};

/// All legal moves in the current position.
pub fn generate_legal(position: &mut Position) -> MoveList {
    let pseudo = generate_pseudo(position, false);
    filter_legal(position, pseudo)
}

/// Legal captures, promotions and en-passant captures. Used by quiescence.
pub fn generate_captures(position: &mut Position) -> MoveList {
    let pseudo = generate_pseudo(position, true);
    filter_legal(position, pseudo)
}

/// Is there at least one legal move? Stops at the first one found.
pub fn has_legal_move(position: &mut Position) -> bool {
    !generate_legal(position).is_empty()
}

fn filter_legal(position: &mut Position, pseudo: MoveList) -> MoveList {
    let us = position.side_to_move();
    let mut legal = MoveList::new();
    for &mv in pseudo.iter() {
        let undo = position.make_move(mv);
        // After the move it is the opponent's turn, so anything attacking our
        // king is an illegal move.
        let safe = !position.is_attacked(position.king_square(us), position.side_to_move());
        position.unmake_move(mv, undo);
        if safe {
            legal.push(mv);
        }
    }
    legal
}

fn generate_pseudo(position: &Position, captures_only: bool) -> MoveList {
    let mut moves = MoveList::new();
    let us = position.side_to_move();
    let them = us.opposite();
    let own = position.color_occupancy(us);
    let enemy = position.color_occupancy(them);
    let occupied = own | enemy;
    let empty = !occupied;
    let king = position.pieces(PieceType::King);

    generate_pawn_moves(position, us, own, enemy, empty, captures_only, &mut moves);

    for from in position.pieces(PieceType::Knight) & own {
        let targets = attacks::knight_attacks(from) & !own & !king;
        push_targets(&mut moves, enemy, from, targets, captures_only);
    }

    for from in position.pieces(PieceType::Bishop) & own {
        let targets = attacks::bishop_attacks(from, occupied) & !own & !king;
        push_targets(&mut moves, enemy, from, targets, captures_only);
    }

    for from in position.pieces(PieceType::Rook) & own {
        let targets = attacks::rook_attacks(from, occupied) & !own & !king;
        push_targets(&mut moves, enemy, from, targets, captures_only);
    }

    for from in position.pieces(PieceType::Queen) & own {
        let targets = attacks::queen_attacks(from, occupied) & !own & !king;
        push_targets(&mut moves, enemy, from, targets, captures_only);
    }

    for from in king & own {
        let targets = attacks::king_attacks(from) & !own;
        push_targets(&mut moves, enemy, from, targets, captures_only);
    }

    if !captures_only {
        generate_castling(position, us, &mut moves);
    }

    moves
}

fn push_targets(
    moves: &mut MoveList,
    enemy: Bitboard,
    from: Square,
    targets: Bitboard,
    captures_only: bool,
) {
    for to in targets {
        let is_capture = enemy.is_set(to);
        if captures_only && !is_capture {
            continue;
        }
        let flag = if is_capture {
            MoveFlag::Capture
        } else {
            MoveFlag::Quiet
        };
        moves.push(Move::new(from, to, flag));
    }
}

fn generate_pawn_moves(
    position: &Position,
    us: Color,
    own: Bitboard,
    enemy: Bitboard,
    empty: Bitboard,
    captures_only: bool,
    moves: &mut MoveList,
) {
    let forward = us.forward();
    let promotion_rank: u8 = if us == Color::White { 7 } else { 0 };
    let double_push_rank: u8 = if us == Color::White { 1 } else { 6 };
    let king = position.pieces(PieceType::King);
    let pawns = position.pieces(PieceType::Pawn) & own;

    for from in pawns {
        let from_rank = from.rank() as i8;

        // Pushes. A quiet promotion is a forcing move, so it is generated in
        // captures-only mode too.
        let one = Square::new(from.file(), (from_rank + forward) as u8);
        if empty.is_set(one) {
            if one.rank() == promotion_rank {
                push_promotions(moves, from, one, false);
            } else if !captures_only {
                moves.push(Move::new(from, one, MoveFlag::Quiet));
                if from.rank() == double_push_rank {
                    let two = Square::new(from.file(), (from_rank + 2 * forward) as u8);
                    if empty.is_set(two) {
                        moves.push(Move::new(from, two, MoveFlag::DoublePush));
                    }
                }
            }
        }

        // Captures, including captures that promote.
        for to in attacks::pawn_attacks(us, from) & enemy & !king {
            if to.rank() == promotion_rank {
                push_promotions(moves, from, to, true);
            } else {
                moves.push(Move::new(from, to, MoveFlag::Capture));
            }
        }

        // En passant. The captured pawn is not on the target square, so this
        // cannot be folded into the loop above.
        if let Some(ep) = position.ep_square() {
            if attacks::pawn_attacks(us, from).is_set(ep) {
                moves.push(Move::new(from, ep, MoveFlag::EnPassant));
            }
        }
    }
}

fn push_promotions(moves: &mut MoveList, from: Square, to: Square, is_capture: bool) {
    for kind in [
        PieceType::Queen,
        PieceType::Rook,
        PieceType::Bishop,
        PieceType::Knight,
    ] {
        if let Some(flag) = MoveFlag::for_promotion(kind, is_capture) {
            moves.push(Move::new(from, to, flag));
        }
    }
}

fn generate_castling(position: &Position, us: Color, moves: &mut MoveList) {
    let rights = position.castling();
    if rights.is_empty() {
        return;
    }
    let them = us.opposite();
    let rank = if us == Color::White { 0 } else { 7 };
    let king_home = Square::new(4, rank);
    if position.king_square(us) != king_home {
        return;
    }
    // You may not castle out of check.
    if position.is_attacked(king_home, them) {
        return;
    }

    let occupied = position.occupancy();

    if rights.has(CastlingRights::king_side(us)) {
        let rook_home = Square::new(7, rank);
        if position.piece_on(rook_home) == Some(Piece::new(us, PieceType::Rook)) {
            let pass = Square::new(5, rank);
            let target = Square::new(6, rank);
            if !occupied.is_set(pass)
                && !occupied.is_set(target)
                && !position.is_attacked(pass, them)
                && !position.is_attacked(target, them)
            {
                moves.push(Move::new(king_home, target, MoveFlag::KingCastle));
            }
        }
    }

    if rights.has(CastlingRights::queen_side(us)) {
        let rook_home = Square::new(0, rank);
        if position.piece_on(rook_home) == Some(Piece::new(us, PieceType::Rook)) {
            let pass = Square::new(3, rank);
            let target = Square::new(2, rank);
            let corner_side = Square::new(1, rank);
            if !occupied.is_set(pass)
                && !occupied.is_set(target)
                && !occupied.is_set(corner_side)
                && !position.is_attacked(pass, them)
                && !position.is_attacked(target, them)
            {
                moves.push(Move::new(king_home, target, MoveFlag::QueenCastle));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn moves_for(fen: &str) -> Vec<String> {
        let mut position = Position::from_fen(fen).expect("valid fen");
        let mut names: Vec<String> = generate_legal(&mut position)
            .iter()
            .map(|mv| mv.to_uci())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn startpos_has_twenty_moves() {
        assert_eq!(moves_for(crate::position::STARTPOS_FEN).len(), 20);
    }

    #[test]
    fn castling_requires_clear_path_and_safe_squares() {
        // White may castle both ways.
        let both = moves_for("r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1");
        assert!(both.contains(&"e1g1".to_string()));
        assert!(both.contains(&"e1c1".to_string()));

        // A bishop on f1 blocks king side; a knight on b1 blocks queen side.
        let blocked = moves_for("r3k2r/8/8/8/8/8/8/RN2KB1R w KQkq - 0 1");
        assert!(!blocked.contains(&"e1g1".to_string()));
        assert!(!blocked.contains(&"e1c1".to_string()));
    }

    #[test]
    fn cannot_castle_through_attacked_square() {
        // Black rook on f8 covers f1, so the king may not pass through it.
        let moves = moves_for("4kr2/8/8/8/8/8/8/R3K2R w KQ - 0 1");
        assert!(!moves.contains(&"e1g1".to_string()));
        assert!(moves.contains(&"e1c1".to_string()));
    }

    #[test]
    fn cannot_castle_out_of_check() {
        let moves = moves_for("4k3/8/8/8/8/8/4r3/R3K2R w KQ - 0 1");
        assert!(!moves.iter().any(|mv| mv == "e1g1" || mv == "e1c1"));
    }

    #[test]
    fn en_passant_is_generated() {
        let moves = moves_for("4k3/8/8/3pP3/8/8/8/4K3 w - d6 0 2");
        assert!(moves.contains(&"e5d6".to_string()));
    }

    #[test]
    fn promotions_offer_four_choices() {
        let moves = moves_for("4k3/2P5/8/8/8/8/8/4K3 w - - 0 1");
        for suffix in ["q", "r", "b", "n"] {
            assert!(
                moves.contains(&format!("c7c8{suffix}")),
                "missing c7c8{suffix}"
            );
        }
        // Capturing promotions also offer four choices.
        let captures = moves_for("1r2k3/2P5/8/8/8/8/8/4K3 w - - 0 1");
        for suffix in ["q", "r", "b", "n"] {
            assert!(
                captures.contains(&format!("c7b8{suffix}")),
                "missing c7b8{suffix}"
            );
        }
    }

    #[test]
    fn pinned_pieces_cannot_move() {
        // The knight on e2 is pinned against the king by the rook on e8.
        let moves = moves_for("4r2k/8/8/8/8/8/4N3/4K3 w - - 0 1");
        assert!(
            !moves.iter().any(|mv| mv.starts_with("e2")),
            "pinned knight moved: {moves:?}"
        );
        // The king itself may still step off the file.
        assert!(moves.iter().any(|mv| mv == "e1d1"));
    }

    #[test]
    fn checkmate_has_no_moves() {
        // Fool's mate.
        let mut position =
            Position::from_fen("rnb1kbnr/pppp1ppp/8/4p3/6Pq/5P2/PPPPP2P/RNBQKBNR w KQkq - 1 3")
                .expect("valid fen");
        assert!(generate_legal(&mut position).is_empty());
        assert!(position.in_check());
    }
}
