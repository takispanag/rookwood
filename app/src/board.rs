//! The board widget.
//!
//! Geometry is kept in one place: [`square_rect`] maps a [`Square`] to pixels
//! and [`square_at`] goes back again, both honouring the flip flag. The widget
//! paints the position and turns mouse gestures into moves; it never mutates
//! the game itself, it returns the move it wants played.

use crate::pieces::draw_piece;
use crate::theme::Theme;
use eframe::egui::{
    Align2, Color32, CornerRadius, CursorIcon, FontId, Pos2, Rect, Response, Sense, Stroke, Ui,
    Vec2,
};
use rookwood::{Game, Move, PieceType, Square};

/// A promotion waiting for the player to choose a piece.
#[derive(Clone, Debug)]
pub struct PendingPromotion {
    pub from: Square,
    pub to: Square,
    pub options: Vec<Move>,
}

impl PendingPromotion {
    /// The move that promotes to `kind`, if it is one of the offered options.
    pub fn with_piece(&self, kind: PieceType) -> Option<Move> {
        self.options
            .iter()
            .copied()
            .find(|mv| mv.promotion() == Some(kind))
    }
}

/// Everything the board widget needs to remember between frames.
#[derive(Clone, Debug)]
pub struct BoardState {
    pub flipped: bool,
    pub selected: Option<Square>,
    pub targets: Vec<Move>,
    pub drag: Option<Square>,
    pub last_move: Option<Move>,
    pub hover: Option<Square>,
    pub pending_promotion: Option<PendingPromotion>,
    pub show_legal: bool,
    pub show_coordinates: bool,
}

impl Default for BoardState {
    fn default() -> Self {
        BoardState {
            flipped: false,
            selected: None,
            targets: Vec::new(),
            drag: None,
            last_move: None,
            hover: None,
            pending_promotion: None,
            show_legal: true,
            show_coordinates: true,
        }
    }
}

impl BoardState {
    /// Select `square` and collect the moves that start there.
    pub fn select(&mut self, game: &mut Game, square: Square) {
        let moves = game.legal_moves();
        self.select_from(&moves, square);
    }

    fn select_from(&mut self, moves: &[Move], square: Square) {
        self.selected = Some(square);
        self.targets = moves
            .iter()
            .copied()
            .filter(|mv| mv.from() == square)
            .collect();
    }

    pub fn clear_selection(&mut self) {
        self.selected = None;
        self.targets.clear();
        self.drag = None;
    }

    pub fn reset(&mut self) {
        self.clear_selection();
        self.last_move = None;
        self.pending_promotion = None;
        self.hover = None;
    }

    /// Turn a from/to pair into a move, opening the promotion picker when the
    /// pawn has more than one option.
    fn resolve(&mut self, from: Square, to: Square) -> Option<Move> {
        let candidates: Vec<Move> = self
            .targets
            .iter()
            .copied()
            .filter(|mv| mv.from() == from && mv.to() == to)
            .collect();
        match candidates.len() {
            0 => None,
            1 => Some(candidates[0]),
            _ => {
                self.pending_promotion = Some(PendingPromotion {
                    from,
                    to,
                    options: candidates,
                });
                None
            }
        }
    }
}

/// Paint the board and handle input. Returns the move the player made, if any.
pub fn show(
    ui: &mut Ui,
    game: &mut Game,
    state: &mut BoardState,
    theme: &Theme,
    interactive: bool,
    side: f32,
) -> Option<Move> {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(side), Sense::click_and_drag());
    let painter = ui.painter_at(rect.expand(10.0));

    paint_board(&painter, rect, game, state, theme);
    let chosen = if interactive {
        handle_input(ui, rect, &response, game, state)
    } else {
        None
    };
    paint_pieces(&painter, rect, game, state, theme);

    if interactive {
        if let Some(square) = state.hover {
            if game.position().piece_on(square).is_some() {
                ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
            }
        }
    }

    chosen
}

fn paint_board(
    painter: &eframe::egui::Painter,
    rect: Rect,
    game: &Game,
    state: &BoardState,
    theme: &Theme,
) {
    painter.rect_filled(rect.expand(6.0), CornerRadius::same(6), theme.board_edge);

    for rank in 0..8u8 {
        for file in 0..8u8 {
            let square = Square::new(file, rank);
            let cell = square_rect(rect, square, state.flipped);
            let light = (file + rank) % 2 == 1;
            let color = if light {
                theme.light_square
            } else {
                theme.dark_square
            };
            painter.rect_filled(cell, CornerRadius::ZERO, color);
        }
    }

    if let Some(mv) = state.last_move {
        for square in [mv.from(), mv.to()] {
            painter.rect_filled(
                square_rect(rect, square, state.flipped),
                CornerRadius::ZERO,
                theme.last_move,
            );
        }
    }
    if let Some(square) = state.selected {
        painter.rect_filled(
            square_rect(rect, square, state.flipped),
            CornerRadius::ZERO,
            theme.selected,
        );
    }

    // Highlight the king when it is in check.
    if game.position().in_check() {
        let king = game.position().king_square(game.position().side_to_move());
        painter.rect_filled(
            square_rect(rect, king, state.flipped),
            CornerRadius::ZERO,
            theme.check,
        );
    }

    if state.show_legal {
        let size = rect.width() / 8.0;
        for mv in &state.targets {
            let cell = square_rect(rect, mv.to(), state.flipped);
            let occupied = game.position().piece_on(mv.to()).is_some() || mv.is_en_passant();
            if occupied {
                painter.circle_stroke(
                    cell.center(),
                    size * 0.42,
                    Stroke::new((size * 0.055).max(1.5), theme.legal_capture),
                );
            } else {
                painter.circle_filled(cell.center(), size * 0.115, theme.legal_dot);
            }
        }
    }

    if let Some(square) = state.hover {
        let cell = square_rect(rect, square, state.flipped);
        painter.rect_filled(cell, CornerRadius::ZERO, Color32::from_white_alpha(18));
    }

    if state.show_coordinates {
        paint_coordinates(painter, rect, state.flipped, theme);
    }
}

fn paint_coordinates(painter: &eframe::egui::Painter, rect: Rect, flipped: bool, theme: &Theme) {
    let size = rect.width() / 8.0;
    let font = FontId::proportional((size * 0.15).max(9.0));
    for index in 0..8u8 {
        let file = if flipped { 7 - index } else { index };
        let rank = if flipped { index } else { 7 - index };

        let file_square = Square::new(file, if flipped { 7 } else { 0 });
        let cell = square_rect(rect, file_square, flipped);
        let dark_text = (file + file_square.rank()) % 2 == 1;
        painter.text(
            cell.right_bottom() - Vec2::splat(size * 0.07),
            Align2::RIGHT_BOTTOM,
            (b'a' + file) as char,
            font.clone(),
            if dark_text {
                theme.light_square
            } else {
                theme.dark_square
            },
        );

        let rank_square = Square::new(if flipped { 7 } else { 0 }, rank);
        let cell = square_rect(rect, rank_square, flipped);
        let dark_text = (rank_square.file() + rank) % 2 == 1;
        painter.text(
            cell.left_top() + Vec2::splat(size * 0.07),
            Align2::LEFT_TOP,
            (b'1' + rank) as char,
            font.clone(),
            if dark_text {
                theme.light_square
            } else {
                theme.dark_square
            },
        );
    }
}

fn paint_pieces(
    painter: &eframe::egui::Painter,
    rect: Rect,
    game: &Game,
    state: &BoardState,
    theme: &Theme,
) {
    let size = rect.width() / 8.0;
    for index in 0..64u8 {
        let square = Square::from_index(index);
        let Some(piece) = game.position().piece_on(square) else {
            continue;
        };
        if state.drag == Some(square) {
            continue;
        }
        let cell = square_rect(rect, square, state.flipped).shrink(size * 0.02);
        draw_piece(painter, cell, piece, theme);
    }

    // The piece being dragged follows the cursor.
    if let (Some(from), Some(pointer)) = (state.drag, state.hover_position(painter.ctx())) {
        if let Some(piece) = game.position().piece_on(from) {
            let cell = Rect::from_center_size(pointer, Vec2::splat(size * 0.96));
            draw_piece(painter, cell, piece, theme);
        }
    }
}

fn handle_input(
    ui: &mut Ui,
    rect: Rect,
    response: &Response,
    game: &mut Game,
    state: &mut BoardState,
) -> Option<Move> {
    let pointer = response
        .hover_pos()
        .or_else(|| response.interact_pointer_pos());
    state.hover = pointer.and_then(|pos| square_at(rect, pos, state.flipped));

    let mut chosen = None;

    if response.drag_started() {
        if let Some(square) = state.hover {
            let piece = game.position().piece_on(square);
            if piece.is_some_and(|piece| piece.color == game.position().side_to_move()) {
                state.select(game, square);
                state.drag = Some(square);
            }
        }
    }

    if response.drag_stopped() {
        if let Some(from) = state.drag.take() {
            if let Some(to) = state.hover {
                if from != to {
                    chosen = state.resolve(from, to);
                }
            }
        }
    }

    if response.clicked() {
        if let Some(square) = state.hover {
            match state.selected {
                Some(from) if from != square => {
                    chosen = state.resolve(from, square);
                    if chosen.is_none() {
                        // Clicking a piece of our own re-targets the selection;
                        // clicking empty space clears it.
                        let own = game
                            .position()
                            .piece_on(square)
                            .is_some_and(|piece| piece.color == game.position().side_to_move());
                        if own {
                            state.select(game, square);
                        } else {
                            state.clear_selection();
                        }
                    }
                }
                Some(_) => state.clear_selection(),
                None => {
                    let own = game
                        .position()
                        .piece_on(square)
                        .is_some_and(|piece| piece.color == game.position().side_to_move());
                    if own {
                        state.select(game, square);
                    }
                }
            }
        }
    }

    if chosen.is_some() {
        state.clear_selection();
    }
    let _ = ui;
    chosen
}

/// Pixel rectangle of `square` inside `board`.
pub fn square_rect(board: Rect, square: Square, flipped: bool) -> Rect {
    let size = board.width() / 8.0;
    let column = if flipped {
        7 - square.file()
    } else {
        square.file()
    } as f32;
    let row = if flipped {
        square.rank()
    } else {
        7 - square.rank()
    } as f32;
    Rect::from_min_size(
        Pos2::new(board.left() + column * size, board.top() + row * size),
        Vec2::splat(size),
    )
}

/// Which square is under `pos`, if any.
pub fn square_at(board: Rect, pos: Pos2, flipped: bool) -> Option<Square> {
    if !board.contains(pos) {
        return None;
    }
    let size = board.width() / 8.0;
    let column = ((pos.x - board.left()) / size).floor().clamp(0.0, 7.0) as u8;
    let row = ((pos.y - board.top()) / size).floor().clamp(0.0, 7.0) as u8;
    let file = if flipped { 7 - column } else { column };
    let rank = if flipped { row } else { 7 - row };
    Some(Square::new(file, rank))
}

impl BoardState {
    fn hover_position(&self, ctx: &eframe::egui::Context) -> Option<Pos2> {
        ctx.input(|input| input.pointer.interact_pos())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn board() -> Rect {
        Rect::from_min_size(Pos2::new(100.0, 50.0), Vec2::splat(400.0))
    }

    #[test]
    fn geometry_round_trips() {
        let board = board();
        for index in 0..64u8 {
            let square = Square::from_index(index);
            for flipped in [false, true] {
                let rect = square_rect(board, square, flipped);
                assert_eq!(square_at(board, rect.center(), flipped), Some(square));
            }
        }
    }

    #[test]
    fn a1_is_bottom_left_when_not_flipped() {
        let board = board();
        let a1 = square_rect(board, Square::from_uci("a1").unwrap(), false);
        assert!(a1.left() < board.left() + 1.0);
        assert!(a1.bottom() > board.bottom() - 1.0);
        let h8 = square_rect(board, Square::from_uci("h8").unwrap(), false);
        assert!(h8.right() > board.right() - 1.0);
        assert!(h8.top() < board.top() + 1.0);
    }

    #[test]
    fn clicking_outside_the_board_is_none() {
        let board = board();
        assert_eq!(square_at(board, Pos2::new(0.0, 0.0), false), None);
        assert_eq!(square_at(board, Pos2::new(600.0, 600.0), false), None);
    }
}
