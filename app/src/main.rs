//! Rookwood's desktop board.
//!
//! The window is a thin shell around the engine crate: the board widget owns
//! the mouse handling, the side panel owns the controls, and the search runs on
//! a worker thread so the UI stays responsive while the engine thinks.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod board;
mod engine;
mod pieces;
mod theme;

use board::BoardState;
use eframe::egui::{
    self, Align2, Color32, CornerRadius, Key, RichText, ScrollArea, Sense, Slider, TextEdit, Ui,
    Vec2,
};
use engine::{EngineHandle, SearchJob, SearchKind, SearchOutcome};
use rookwood::{Color, ENGINE_VERSION, Game, GameStatus, Move, PieceType, SearchLimits};
use std::time::Duration;
use theme::Theme;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1140.0, 800.0])
            .with_min_inner_size([900.0, 640.0])
            .with_title("Rookwood"),
        ..Default::default()
    };
    eframe::run_native(
        "Rookwood",
        options,
        Box::new(|cc| Ok(Box::new(RookwoodApp::new(cc)))),
    )
}

#[derive(Clone, Debug)]
struct Settings {
    /// Let the engine play one side.
    engine_plays: bool,
    engine_color: Color,
    depth: u8,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            engine_plays: true,
            engine_color: Color::Black,
            depth: 4,
        }
    }
}

struct RookwoodApp {
    game: Game,
    board: BoardState,
    engine: EngineHandle,
    theme: Theme,
    settings: Settings,
    status: GameStatus,
    analysis: Option<SearchOutcome>,
    generation: u64,
    fen_input: String,
    fen_message: Option<(String, bool)>,
    about_open: bool,
}

impl RookwoodApp {
    fn new(cc: &eframe::CreationContext<'_>) -> RookwoodApp {
        let theme = Theme::default();
        theme.apply(&cc.egui_ctx);
        let game = Game::new();
        let fen_input = game.fen();
        RookwoodApp {
            game,
            board: BoardState::default(),
            engine: EngineHandle::spawn(),
            theme,
            settings: Settings::default(),
            status: GameStatus::Ongoing,
            analysis: None,
            generation: 0,
            fen_input,
            fen_message: None,
            about_open: false,
        }
    }

    // ------------------------------------------------------------ game flow

    fn new_game(&mut self) {
        self.engine.stop();
        self.generation += 1;
        self.game.reset();
        self.on_position_changed();
        self.fen_input = self.game.fen();
        self.fen_message = None;
    }

    fn undo(&mut self) {
        self.engine.stop();
        self.generation += 1;
        if self.game.pop_move().is_none() {
            return;
        }
        // Playing the engine? Take its reply back too, so the player is on move.
        if self.settings.engine_plays && self.game.side_to_move() == self.settings.engine_color {
            self.game.pop_move();
        }
        self.on_position_changed();
    }

    fn load_fen(&mut self) {
        match Game::from_fen(&self.fen_input) {
            Ok(game) => {
                self.engine.stop();
                self.generation += 1;
                self.game = game;
                self.on_position_changed();
                self.fen_input = self.game.fen();
                self.fen_message = Some(("position loaded".to_string(), false));
            }
            Err(error) => self.fen_message = Some((error.to_string(), true)),
        }
    }

    fn on_position_changed(&mut self) {
        self.board.reset();
        self.board.last_move = self.game.moves().last().copied();
        self.analysis = None;
        self.status = self.game.status();
        self.fen_input = self.game.fen();
    }

    fn play_move(&mut self, mv: Move) {
        if !self.game.try_move(mv) {
            return;
        }
        self.board.last_move = Some(mv);
        self.board.clear_selection();
        self.analysis = None;
        self.status = self.game.status();
        self.fen_input = self.game.fen();
        self.fen_message = None;
        self.maybe_let_engine_move();
    }

    fn maybe_let_engine_move(&mut self) {
        if !self.settings.engine_plays
            || self.status.is_over()
            || self.game.side_to_move() != self.settings.engine_color
        {
            return;
        }
        self.start_search(SearchKind::Play);
    }

    fn start_search(&mut self, kind: SearchKind) {
        if self.engine.is_busy() {
            return;
        }
        self.generation += 1;
        let job = SearchJob {
            generation: self.generation,
            game: self.game.clone(),
            limits: SearchLimits::with_depth(self.settings.depth.max(1)),
            kind,
        };
        if !self.engine.submit(job) {
            self.generation -= 1;
        }
    }

    fn poll_engine(&mut self) {
        let Some(outcome) = self.engine.poll() else {
            return;
        };
        if outcome.generation != self.generation {
            return;
        }
        match outcome.kind {
            SearchKind::Analyse => self.analysis = Some(outcome),
            SearchKind::Play => {
                if let Some(mv) = outcome.result.best_move {
                    if self.game.try_move(mv) {
                        self.board.last_move = Some(mv);
                        self.board.clear_selection();
                        self.status = self.game.status();
                        self.fen_input = self.game.fen();
                    }
                }
                self.analysis = Some(outcome);
            }
        }
    }

    fn white_score(&self) -> i32 {
        match &self.analysis {
            Some(outcome) => match self.game.side_to_move() {
                Color::White => outcome.result.score,
                Color::Black => -outcome.result.score,
            },
            None => rookwood::eval::evaluate_white(self.game.position()),
        }
    }

    fn score_text(&self) -> String {
        if let Some(outcome) = &self.analysis {
            if let Some(mate) = outcome.result.mate_in() {
                return if mate > 0 {
                    format!("mate in {mate}")
                } else {
                    format!("mated in {}", -mate)
                };
            }
        }
        format!("{:+.2}", self.white_score() as f32 / 100.0)
    }

    fn status_text(&self) -> String {
        match self.status {
            GameStatus::Ongoing => {
                if self.game.position().in_check() {
                    "check".to_string()
                } else {
                    format!("{} to move", self.game.side_to_move())
                }
            }
            GameStatus::Checkmate => {
                let winner = self.game.side_to_move().opposite();
                format!("checkmate — {winner} wins")
            }
            GameStatus::Stalemate => "stalemate".to_string(),
            GameStatus::Draw(reason) => reason.description().to_string(),
        }
    }

    // -------------------------------------------------------------- widgets

    fn side_panel(&mut self, ui: &mut Ui) {
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.heading(RichText::new("Rookwood").color(self.theme.accent));
            ui.label(RichText::new(ENGINE_VERSION).small().weak());
        });
        ui.label(
            RichText::new("chess engine · analysis board")
                .small()
                .weak(),
        );
        ui.add_space(6.0);

        let status_color = match self.status {
            GameStatus::Ongoing => self.theme.text_dim,
            GameStatus::Checkmate => self.theme.bad,
            _ => self.theme.accent,
        };
        ui.label(
            RichText::new(self.status_text())
                .color(status_color)
                .strong(),
        );
        if self.engine.is_busy() {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(RichText::new("thinking…").small().weak());
            });
        }
        ui.separator();

        ui.horizontal(|ui| {
            if ui.button("New game").clicked() {
                self.new_game();
            }
            if ui.button("Flip").clicked() {
                self.board.flipped = !self.board.flipped;
            }
            if ui.button("Undo").clicked() {
                self.undo();
            }
        });

        ui.add_space(4.0);
        self.eval_bar(ui);
        ui.add_space(2.0);
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(self.score_text())
                    .monospace()
                    .color(self.theme.eval_color(self.white_score())),
            );
            if self.board.flipped {
                ui.label(RichText::new("black at the bottom").small().weak());
            }
        });
        ui.separator();

        self.engine_controls(ui);
        ui.separator();
        self.analysis_panel(ui);
        ui.separator();
        self.move_list(ui);
        ui.separator();
        self.fen_box(ui);

        ui.add_space(6.0);
        ui.horizontal(|ui| {
            if ui
                .selectable_label(self.about_open, RichText::new("About").small().weak())
                .clicked()
            {
                self.about_open = !self.about_open;
            }
        });
    }

    fn eval_bar(&self, ui: &mut Ui) {
        let width = ui.available_width();
        let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 14.0), Sense::hover());
        let painter = ui.painter();
        painter.rect_filled(rect, CornerRadius::same(4), Color32::from_rgb(46, 40, 35));
        let share = ((self.white_score() as f32 / 800.0) + 0.5).clamp(0.03, 0.97);
        let filled =
            egui::Rect::from_min_size(rect.min, Vec2::new(rect.width() * share, rect.height()));
        painter.rect_filled(filled, CornerRadius::same(4), self.theme.white_piece);
        painter.rect_stroke(
            rect,
            CornerRadius::same(4),
            egui::Stroke::new(1.0, Color32::from_rgb(70, 62, 55)),
            egui::StrokeKind::Inside,
        );
    }

    fn engine_controls(&mut self, ui: &mut Ui) {
        ui.label(RichText::new("ENGINE").small().weak().strong());
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.settings.engine_plays, "Plays");
            ui.add_enabled_ui(self.settings.engine_plays, |ui| {
                ui.selectable_value(&mut self.settings.engine_color, Color::White, "White");
                ui.selectable_value(&mut self.settings.engine_color, Color::Black, "Black");
            });
        });
        ui.horizontal(|ui| {
            ui.label("Depth");
            ui.add(
                Slider::new(&mut self.settings.depth, 1..=8).clamping(egui::SliderClamping::Always),
            );
        });

        ui.horizontal(|ui| {
            let can_analyse = !self.engine.is_busy() && !self.status.is_over();
            if ui
                .add_enabled(can_analyse, egui::Button::new("Analyse"))
                .clicked()
            {
                self.start_search(SearchKind::Analyse);
            }
            if ui
                .add_enabled(self.engine.is_busy(), egui::Button::new("Stop"))
                .clicked()
            {
                self.engine.stop();
            }
        });
    }

    fn analysis_panel(&mut self, ui: &mut Ui) {
        ui.add_space(4.0);
        ui.label(RichText::new("ANALYSIS").small().weak().strong());
        match &self.analysis {
            Some(outcome) => {
                let result = &outcome.result;
                ui.label(
                    RichText::new(format!(
                        "depth {} · {} nodes · {:.1} Mnps · {:.2}s",
                        result.depth,
                        result.nodes,
                        result.nodes_per_second() as f64 / 1_000_000.0,
                        result.elapsed.as_secs_f64()
                    ))
                    .small()
                    .weak(),
                );
                if result.interrupted {
                    ui.label(
                        RichText::new("stopped early")
                            .small()
                            .color(self.theme.accent),
                    );
                }
                let pv: Vec<String> = result.pv.iter().map(|mv| mv.to_uci()).collect();
                if !pv.is_empty() {
                    ui.add(
                        egui::Label::new(RichText::new(pv.join(" ")).monospace().small())
                            .wrap_mode(egui::TextWrapMode::Wrap),
                    );
                }
            }
            None => {
                ui.label(
                    RichText::new("no analysis yet — press Analyse, or make a move")
                        .small()
                        .weak(),
                );
            }
        }
        ui.add_space(4.0);
    }

    fn move_list(&mut self, ui: &mut Ui) {
        ui.label(RichText::new("MOVES").small().weak().strong());
        let moves: Vec<String> = self.game.moves().iter().map(|mv| mv.to_uci()).collect();
        ScrollArea::vertical()
            .max_height(150.0)
            .stick_to_bottom(true)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                if moves.is_empty() {
                    ui.label(RichText::new("the game starts here").small().weak());
                    return;
                }
                egui::Grid::new("move-grid")
                    .num_columns(3)
                    .striped(true)
                    .spacing([10.0, 2.0])
                    .show(ui, |ui| {
                        for (index, pair) in moves.chunks(2).enumerate() {
                            ui.label(RichText::new(format!("{}.", index + 1)).weak().small());
                            ui.label(RichText::new(&pair[0]).monospace());
                            match pair.get(1) {
                                Some(reply) => ui.label(RichText::new(reply).monospace()),
                                None => ui.label(""),
                            };
                            ui.end_row();
                        }
                    });
            });
    }

    fn fen_box(&mut self, ui: &mut Ui) {
        ui.label(RichText::new("POSITION (FEN)").small().weak().strong());
        ui.add(
            TextEdit::singleline(&mut self.fen_input)
                .desired_width(f32::INFINITY)
                .font(egui::TextStyle::Monospace),
        );
        ui.horizontal(|ui| {
            if ui.button("Load FEN").clicked() {
                self.load_fen();
            }
            if ui.button("Copy").clicked() {
                ui.ctx().copy_text(self.fen_input.clone());
            }
        });
        if let Some((message, is_error)) = &self.fen_message {
            let color = if *is_error {
                self.theme.bad
            } else {
                self.theme.good
            };
            ui.label(RichText::new(message).small().color(color));
        }
    }

    fn board_panel(&mut self, ui: &mut Ui) {
        let engine_turn = self.settings.engine_plays
            && !self.status.is_over()
            && self.game.side_to_move() == self.settings.engine_color;
        let interactive = !engine_turn && !self.engine.is_busy();

        ui.add_space(2.0);
        // The board gets a square that fits both the panel width and whatever
        // height is left after the row of checkboxes underneath.
        let available = ui.available_rect_before_wrap();
        let side = (available.width().min(available.height() - 46.0)).clamp(160.0, 1400.0);

        ui.horizontal(|ui| {
            let padding = ((ui.available_width() - side) * 0.5).max(0.0);
            ui.add_space(padding);
            if let Some(mv) = board::show(
                ui,
                &mut self.game,
                &mut self.board,
                &self.theme,
                interactive,
                side,
            ) {
                self.play_move(mv);
            }
        });

        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.board.show_legal, "Show legal moves");
            ui.checkbox(&mut self.board.show_coordinates, "Coordinates");
            if engine_turn {
                ui.label(RichText::new("engine's move").small().weak());
            }
        });
    }

    fn promotion_window(&mut self, ctx: &egui::Context) {
        let Some(pending) = self.board.pending_promotion.clone() else {
            return;
        };
        let side = self.game.side_to_move();
        let mut choice = None;

        egui::Window::new("Promote to")
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label(
                    RichText::new(format!("{} → {}", pending.from, pending.to))
                        .small()
                        .weak()
                        .monospace(),
                );
                ui.horizontal(|ui| {
                    for kind in [
                        PieceType::Queen,
                        PieceType::Rook,
                        PieceType::Bishop,
                        PieceType::Knight,
                    ] {
                        let (rect, response) =
                            ui.allocate_exact_size(Vec2::splat(56.0), Sense::click());
                        if response.hovered() {
                            ui.painter().rect_filled(
                                rect,
                                CornerRadius::same(6),
                                Color32::from_white_alpha(18),
                            );
                        }
                        pieces::draw_piece_icon(
                            ui.painter(),
                            rect.shrink(8.0),
                            side,
                            kind,
                            &self.theme,
                        );
                        if response.clicked() {
                            choice = Some(kind);
                        }
                    }
                });
            });

        if let Some(kind) = choice {
            if let Some(mv) = pending.with_piece(kind) {
                self.play_move(mv);
            }
            self.board.pending_promotion = None;
        }
    }

    fn about_window(&mut self, ctx: &egui::Context) {
        if !self.about_open {
            return;
        }
        let mut open = self.about_open;
        egui::Window::new("About Rookwood")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label(RichText::new(format!("Rookwood {ENGINE_VERSION}")).strong());
                ui.label("A chess engine and analysis board built from scratch in Rust.");
                ui.add_space(4.0);
                ui.label(
                    RichText::new(
                        "Move generation, FEN, the draw rules, evaluation, alpha-beta search and \
                         the UCI protocol are all original code. The board is drawn with vector \
                         shapes — there are no image assets anywhere in the project.",
                    )
                    .small(),
                );
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Shortcuts:").small().strong());
                    ui.label(RichText::new("F flip · Ctrl+Z undo").small().weak());
                });
            });
        self.about_open = open;
    }
}

impl eframe::App for RookwoodApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.poll_engine();
        self.status = self.game.status();

        if ui.input(|input| input.key_pressed(Key::F)) {
            self.board.flipped = !self.board.flipped;
        }
        if ui.input(|input| input.key_pressed(Key::Z) && input.modifiers.command) {
            self.undo();
        }

        egui::Panel::right("controls")
            .default_size(330.0)
            .resizable(false)
            .show(ui, |ui| self.side_panel(ui));

        egui::CentralPanel::default().show(ui, |ui| self.board_panel(ui));

        self.promotion_window(ui.ctx());
        self.about_window(ui.ctx());

        // Keep the clock/analysis display alive while a search is running.
        if self.engine.is_busy() {
            ui.ctx().request_repaint_after(Duration::from_millis(80));
        }
    }
}
