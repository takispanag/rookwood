//! Colours and egui styling.
//!
//! The palette is deliberately warm: oak and walnut squares, brass accents,
//! cream and charcoal pieces. Nothing here comes from an asset file — every
//! colour is defined in code.

use eframe::egui::{Color32, Stroke, Visuals};
use rookwood::Color;

#[derive(Clone, Debug)]
pub struct Theme {
    pub light_square: Color32,
    pub dark_square: Color32,
    pub board_edge: Color32,
    pub last_move: Color32,
    pub selected: Color32,
    pub legal_dot: Color32,
    pub legal_capture: Color32,
    pub check: Color32,
    pub white_piece: Color32,
    pub white_edge: Color32,
    pub black_piece: Color32,
    pub black_edge: Color32,
    pub accent: Color32,
    pub text_dim: Color32,
    pub panel: Color32,
    pub good: Color32,
    pub bad: Color32,
}

impl Default for Theme {
    fn default() -> Theme {
        Theme {
            light_square: Color32::from_rgb(232, 211, 176),
            dark_square: Color32::from_rgb(166, 116, 79),
            board_edge: Color32::from_rgb(92, 62, 43),
            last_move: Color32::from_rgba_unmultiplied(214, 178, 66, 120),
            selected: Color32::from_rgba_unmultiplied(88, 168, 120, 150),
            legal_dot: Color32::from_rgba_unmultiplied(40, 34, 28, 90),
            legal_capture: Color32::from_rgba_unmultiplied(196, 74, 58, 170),
            check: Color32::from_rgba_unmultiplied(208, 70, 58, 170),
            white_piece: Color32::from_rgb(246, 240, 228),
            white_edge: Color32::from_rgb(58, 43, 33),
            black_piece: Color32::from_rgb(44, 36, 32),
            black_edge: Color32::from_rgb(18, 14, 12),
            accent: Color32::from_rgb(199, 154, 60),
            text_dim: Color32::from_rgb(168, 156, 142),
            panel: Color32::from_rgb(28, 25, 23),
            good: Color32::from_rgb(120, 182, 132),
            bad: Color32::from_rgb(206, 106, 92),
        }
    }
}

impl Theme {
    pub fn apply(&self, ctx: &eframe::egui::Context) {
        let mut visuals = Visuals::dark();
        visuals.panel_fill = self.panel;
        visuals.window_fill = Color32::from_rgb(34, 30, 27);
        visuals.faint_bg_color = Color32::from_rgb(38, 34, 30);
        visuals.extreme_bg_color = Color32::from_rgb(20, 18, 16);
        visuals.override_text_color = Some(Color32::from_rgb(226, 216, 203));
        visuals.selection.bg_fill = self.accent.gamma_multiply(0.45);
        visuals.selection.stroke = Stroke::new(1.0, self.accent);
        visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, Color32::from_rgb(56, 50, 44));
        visuals.widgets.inactive.bg_fill = Color32::from_rgb(46, 41, 36);
        visuals.widgets.hovered.bg_fill = Color32::from_rgb(62, 55, 48);
        visuals.widgets.active.bg_fill = self.accent.gamma_multiply(0.7);
        ctx.set_visuals(visuals);
    }

    /// Fill and outline for a piece of the given colour.
    pub fn piece_colors(&self, color: Color) -> (Color32, Color32) {
        match color {
            Color::White => (self.white_piece, self.white_edge),
            Color::Black => (self.black_piece, self.black_edge),
        }
    }

    /// Colour for an evaluation figure, from White's point of view.
    pub fn eval_color(&self, score: i32) -> Color32 {
        if score > 40 {
            self.good
        } else if score < -40 {
            self.bad
        } else {
            self.text_dim
        }
    }
}
