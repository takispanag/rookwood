//! Piece artwork, drawn with vector shapes at any size.
//!
//! There are no image files in this project: each piece is a small composition
//! of circles, rounded bars and convex polygons expressed in a unit square and
//! scaled to the board. That keeps the repository asset-free, makes the pieces
//! crisp at every zoom level, and means the look is entirely ours.
//!
//! A piece is painted twice: once offset downwards in translucent black for a
//! soft shadow, then normally on top.

use crate::theme::Theme;
use eframe::egui::{Color32, CornerRadius, Painter, Pos2, Rect, Shape, Stroke, Vec2};
use rookwood::{Color, Piece, PieceType};

/// Draw `piece` centred in `rect`.
pub fn draw_piece(painter: &Painter, rect: Rect, piece: Piece, theme: &Theme) {
    let (fill, edge) = theme.piece_colors(piece.color);
    let width = (rect.width() * 0.018).max(1.0);

    let shadow = Pen {
        painter,
        rect,
        fill: Color32::from_black_alpha(52),
        stroke: Stroke::new(width, Color32::from_black_alpha(52)),
        offset: Vec2::new(0.0, rect.height() * 0.018),
    };
    paint(&shadow, piece.kind);

    let pen = Pen {
        painter,
        rect,
        fill,
        stroke: Stroke::new(width, edge),
        offset: Vec2::ZERO,
    };
    paint(&pen, piece.kind);
}

/// Draw a piece into a circle, used by the promotion picker.
pub fn draw_piece_icon(
    painter: &Painter,
    rect: Rect,
    color: Color,
    kind: PieceType,
    theme: &Theme,
) {
    draw_piece(painter, rect, Piece::new(color, kind), theme);
}

fn paint(pen: &Pen<'_>, kind: PieceType) {
    match kind {
        PieceType::Pawn => pawn(pen),
        PieceType::Knight => knight(pen),
        PieceType::Bishop => bishop(pen),
        PieceType::Rook => rook(pen),
        PieceType::Queen => queen(pen),
        PieceType::King => king(pen),
    }
}

/// A tiny immediate-mode drawing helper working in unit coordinates.
struct Pen<'a> {
    painter: &'a Painter,
    rect: Rect,
    fill: Color32,
    stroke: Stroke,
    offset: Vec2,
}

impl Pen<'_> {
    fn at(&self, x: f32, y: f32) -> Pos2 {
        Pos2::new(
            self.rect.left() + x * self.rect.width() + self.offset.x,
            self.rect.top() + y * self.rect.height() + self.offset.y,
        )
    }

    fn poly(&self, points: &[(f32, f32)]) {
        let points = points.iter().map(|&(x, y)| self.at(x, y)).collect();
        self.painter
            .add(Shape::convex_polygon(points, self.fill, self.stroke));
    }

    fn disc(&self, x: f32, y: f32, radius: f32) {
        let centre = self.at(x, y);
        let radius = radius * self.rect.width();
        self.painter.circle_filled(centre, radius, self.fill);
        self.painter.circle_stroke(centre, radius, self.stroke);
    }

    /// A rounded bar with a matching outline, drawn as an underlay.
    fn slab(&self, x0: f32, y0: f32, x1: f32, y1: f32, radius: f32) {
        let grow = self.rect.width() * 0.008;
        let inner = Rect::from_min_max(self.at(x0, y0), self.at(x1, y1));
        let outer = inner.expand(grow);
        let corner = CornerRadius::same((radius * self.rect.width()).clamp(0.0, 255.0) as u8);
        self.painter.rect_filled(outer, corner, self.stroke.color);
        self.painter.rect_filled(inner, corner, self.fill);
    }

    fn line(&self, from: (f32, f32), to: (f32, f32)) {
        let stroke = Stroke::new(self.stroke.width * 0.85, self.stroke.color);
        self.painter
            .line_segment([self.at(from.0, from.1), self.at(to.0, to.1)], stroke);
    }
}

/// The wide foot every piece stands on.
fn base(pen: &Pen<'_>, inset: f32) {
    pen.slab(inset, 0.78, 1.0 - inset, 0.87, 0.03);
}

fn pawn(pen: &Pen<'_>) {
    pen.disc(0.50, 0.35, 0.105);
    pen.slab(0.41, 0.44, 0.59, 0.49, 0.018);
    pen.poly(&[(0.42, 0.47), (0.58, 0.47), (0.62, 0.76), (0.38, 0.76)]);
    base(pen, 0.30);
}

fn knight(pen: &Pen<'_>) {
    // Neck, then the head as one convex silhouette with an ear on top.
    pen.poly(&[(0.40, 0.55), (0.66, 0.55), (0.68, 0.78), (0.34, 0.78)]);
    pen.poly(&[
        (0.50, 0.18),
        (0.62, 0.30),
        (0.74, 0.42),
        (0.70, 0.56),
        (0.40, 0.60),
        (0.36, 0.40),
        (0.42, 0.28),
    ]);
    pen.poly(&[(0.44, 0.28), (0.49, 0.13), (0.55, 0.26)]);
    base(pen, 0.28);
    // Eye.
    pen.disc(0.585, 0.375, 0.021);
    pen.line((0.47, 0.55), (0.60, 0.55));
}

fn bishop(pen: &Pen<'_>) {
    pen.disc(0.50, 0.17, 0.036);
    pen.poly(&[
        (0.50, 0.22),
        (0.62, 0.36),
        (0.60, 0.50),
        (0.50, 0.57),
        (0.40, 0.50),
        (0.38, 0.36),
    ]);
    pen.slab(0.42, 0.56, 0.58, 0.61, 0.018);
    pen.poly(&[(0.43, 0.61), (0.57, 0.61), (0.62, 0.78), (0.38, 0.78)]);
    base(pen, 0.28);
    // The mitre's diagonal slit.
    pen.line((0.50, 0.27), (0.57, 0.41));
}

fn rook(pen: &Pen<'_>) {
    pen.slab(0.27, 0.22, 0.38, 0.35, 0.008);
    pen.slab(0.445, 0.22, 0.555, 0.35, 0.008);
    pen.slab(0.62, 0.22, 0.73, 0.35, 0.008);
    pen.slab(0.27, 0.33, 0.73, 0.41, 0.012);
    pen.poly(&[(0.35, 0.41), (0.65, 0.41), (0.70, 0.78), (0.30, 0.78)]);
    base(pen, 0.26);
}

fn queen(pen: &Pen<'_>) {
    // Three crown spikes, each topped with a pearl.
    pen.poly(&[(0.30, 0.38), (0.36, 0.19), (0.46, 0.38)]);
    pen.poly(&[(0.42, 0.38), (0.50, 0.13), (0.58, 0.38)]);
    pen.poly(&[(0.54, 0.38), (0.64, 0.19), (0.70, 0.38)]);
    pen.slab(0.29, 0.35, 0.71, 0.43, 0.015);
    pen.poly(&[(0.37, 0.43), (0.63, 0.43), (0.70, 0.78), (0.30, 0.78)]);
    base(pen, 0.26);
    pen.disc(0.36, 0.18, 0.030);
    pen.disc(0.50, 0.12, 0.030);
    pen.disc(0.64, 0.18, 0.030);
}

fn king(pen: &Pen<'_>) {
    pen.slab(0.465, 0.13, 0.535, 0.30, 0.012);
    pen.slab(0.395, 0.185, 0.605, 0.245, 0.012);
    pen.slab(0.34, 0.30, 0.66, 0.38, 0.018);
    pen.poly(&[(0.38, 0.38), (0.62, 0.38), (0.70, 0.78), (0.30, 0.78)]);
    base(pen, 0.26);
}
