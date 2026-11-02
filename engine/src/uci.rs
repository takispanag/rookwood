//! The Universal Chess Interface command loop.
//!
//! The loop reads commands from a `BufRead`, writes to a shared `Write`, and
//! runs the search on a worker thread so that `stop` is answered while the
//! engine is still thinking. Everything is generic over the streams, which
//! makes the protocol testable without spawning a process.

use crate::game::Game;
use crate::perft::perft;
use crate::position::Position;
use crate::search::{self, MATE_THRESHOLD, SearchLimits};
use crate::types::Color;
use crate::{ENGINE_NAME, ENGINE_VERSION};
use std::fmt;
use std::io::{self, BufRead, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

/// A running search, and the flag that can interrupt it.
struct ActiveSearch {
    stop: Arc<AtomicBool>,
    handle: JoinHandle<()>,
}

/// The UCI front end.
pub struct UciEngine<W: Write + Send + 'static> {
    game: Game,
    output: Arc<Mutex<W>>,
    search: Option<ActiveSearch>,
    debug: bool,
}

impl<W: Write + Send + 'static> UciEngine<W> {
    pub fn new(output: W) -> UciEngine<W> {
        UciEngine {
            game: Game::new(),
            output: Arc::new(Mutex::new(output)),
            search: None,
            debug: false,
        }
    }

    /// Read commands until `quit` or end of input.
    pub fn run(&mut self, input: impl BufRead) -> io::Result<()> {
        for line in input.lines() {
            let line = line?;
            if !self.handle_command(&line) {
                break;
            }
        }
        self.stop_search();
        Ok(())
    }

    /// Execute one command line. Returns `false` when the engine should exit.
    pub fn handle_command(&mut self, line: &str) -> bool {
        let line = line.trim();
        if line.is_empty() {
            return true;
        }
        let mut tokens = line.split_whitespace();
        let command = tokens.next().unwrap_or_default();

        match command {
            "uci" => {
                self.emit(format_args!("id name {} {}", ENGINE_NAME, ENGINE_VERSION));
                self.emit(format_args!("id author {}", env!("CARGO_PKG_AUTHORS")));
                self.emit(format_args!("option name Clear Hash type button"));
                self.emit(format_args!("uciok"));
            }
            "isready" => self.emit(format_args!("readyok")),
            "debug" => {
                self.debug = tokens.next() == Some("on");
            }
            "setoption" => {
                // No configurable options yet; acknowledge silently so that
                // GUIs which always send `setoption name Hash` keep working.
                if self.debug {
                    self.emit(format_args!("info string ignoring option: {line}"));
                }
            }
            "ucinewgame" => {
                self.stop_search();
                self.game = Game::new();
            }
            "position" => self.handle_position(line),
            "go" => self.handle_go(&line["go".len()..]),
            "stop" => self.interrupt_search(),
            "ponderhit" => self.interrupt_search(),
            "perft" => self.handle_perft(tokens.next()),
            "d" | "board" => self.emit(format_args!("{}", self.game.position())),
            "fen" => self.emit(format_args!("{}", self.game.fen())),
            "eval" => {
                let score = crate::eval::evaluate_white(self.game.position());
                self.emit(format_args!(
                    "info string static evaluation: {score} cp (white)"
                ));
            }
            "quit" => {
                self.stop_search();
                return false;
            }
            other => {
                if self.debug {
                    self.emit(format_args!("info string unknown command: {other}"));
                }
            }
        }
        true
    }

    /// Block until any running search has finished. Mostly useful in tests.
    pub fn wait(&mut self) {
        self.stop_search();
    }

    fn emit(&self, message: fmt::Arguments<'_>) {
        let mut output = self.output.lock().expect("UCI output mutex poisoned");
        let _ = writeln!(output, "{message}");
        let _ = output.flush();
    }

    fn handle_position(&mut self, line: &str) {
        self.stop_search();

        let rest = line["position".len()..].trim();
        let (base, moves) = match rest.find(" moves ") {
            Some(index) => (&rest[..index], &rest[index + " moves ".len()..]),
            None => (rest, ""),
        };

        let mut tokens = base.split_whitespace();
        match tokens.next() {
            Some("startpos") => self.game = Game::new(),
            Some("fen") => {
                let fen = tokens.collect::<Vec<_>>().join(" ");
                match Game::from_fen(&fen) {
                    Ok(game) => self.game = game,
                    Err(error) => {
                        self.emit(format_args!("info string invalid FEN: {error}"));
                        return;
                    }
                }
            }
            _ => {
                self.emit(format_args!("info string position needs startpos or fen"));
                return;
            }
        }

        for text in moves.split_whitespace() {
            if self.game.play_uci(text).is_none() {
                self.emit(format_args!("info string illegal move in position: {text}"));
                break;
            }
        }
    }

    fn handle_go(&mut self, arguments: &str) {
        let limits = match self.parse_go(arguments) {
            Some(limits) => limits,
            None => return,
        };
        self.start_search(limits);
    }

    fn parse_go(&self, arguments: &str) -> Option<SearchLimits> {
        let tokens: Vec<&str> = arguments.split_whitespace().collect();
        let mut depth: Option<u8> = None;
        let mut movetime: Option<u64> = None;
        let mut nodes: Option<u64> = None;
        let mut infinite = false;
        let mut white_time: Option<u64> = None;
        let mut black_time: Option<u64> = None;
        let mut white_inc = 0u64;
        let mut black_inc = 0u64;
        let mut moves_to_go: Option<u64> = None;

        let mut index = 0;
        while index < tokens.len() {
            let value = tokens.get(index + 1).copied();
            match tokens[index] {
                "depth" => depth = value.and_then(|v| v.parse().ok()),
                "movetime" => movetime = value.and_then(|v| v.parse().ok()),
                "nodes" => nodes = value.and_then(|v| v.parse().ok()),
                "infinite" => infinite = true,
                "ponder" => infinite = true,
                "wtime" => white_time = value.and_then(|v| v.parse().ok()),
                "btime" => black_time = value.and_then(|v| v.parse().ok()),
                "winc" => white_inc = value.and_then(|v| v.parse().ok()).unwrap_or(0),
                "binc" => black_inc = value.and_then(|v| v.parse().ok()).unwrap_or(0),
                "movestogo" => moves_to_go = value.and_then(|v| v.parse().ok()),
                "searchmoves" => {
                    self.emit(format_args!("info string searchmoves is not supported"));
                }
                _ => {}
            }
            index += 2;
        }

        // A clock budget beats a depth limit, because a GUI that sends
        // `wtime` expects an answer before it flags.
        let budget = match (self.game.side_to_move(), white_time, black_time) {
            (Color::White, Some(remaining), _) => {
                Some(spend_budget(remaining, white_inc, moves_to_go))
            }
            (Color::Black, _, Some(remaining)) => {
                Some(spend_budget(remaining, black_inc, moves_to_go))
            }
            _ => None,
        };

        let movetime = movetime.or(budget);
        let depth = depth.unwrap_or(if infinite || movetime.is_some() {
            64
        } else {
            6
        });

        Some(SearchLimits {
            depth,
            movetime: movetime.map(Duration::from_millis),
            nodes,
        })
    }

    fn start_search(&mut self, limits: SearchLimits) {
        self.stop_search();

        let mut game = self.game.clone();
        let output = Arc::clone(&self.output);
        let stop = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&stop);
        let debug = self.debug;
        let start = Instant::now();

        let handle = thread::spawn(move || {
            let result = search::search(&mut game, limits, flag);

            let score = match result.mate_in() {
                Some(moves) => format!("mate {moves}"),
                None => format!("cp {}", result.score),
            };
            let pv = result
                .pv
                .iter()
                .map(|mv| mv.to_uci())
                .collect::<Vec<_>>()
                .join(" ");
            let elapsed = start.elapsed().as_millis().max(1) as u64;
            let nps = result.nodes.saturating_mul(1000) / elapsed;

            let mut writer = output.lock().expect("UCI output mutex poisoned");
            let _ = writeln!(
                writer,
                "info depth {} score {} nodes {} nps {} time {} pv {}",
                result.depth, score, result.nodes, nps, elapsed, pv
            );
            if debug {
                let _ = writeln!(
                    writer,
                    "info string {} nodes in {:.2}s ({} nps)",
                    result.nodes,
                    elapsed as f64 / 1000.0,
                    nps
                );
            }
            match result.best_move {
                Some(mv) => {
                    let _ = writeln!(writer, "bestmove {}", mv.to_uci());
                }
                None => {
                    let _ = writeln!(writer, "bestmove 0000");
                }
            }
            let _ = writer.flush();
        });

        self.search = Some(ActiveSearch { stop, handle });
    }

    fn handle_perft(&mut self, depth: Option<&str>) {
        let depth: u32 = match depth.and_then(|value| value.parse().ok()) {
            Some(depth) => depth,
            None => {
                self.emit(format_args!("info string perft needs a depth"));
                return;
            }
        };
        let mut position = self.game.position().clone();
        let start = Instant::now();
        let nodes = perft(&mut position, depth);
        let elapsed = start.elapsed();
        self.emit(format_args!(
            "perft {depth} nodes {nodes} time {}ms",
            elapsed.as_millis()
        ));
    }

    fn interrupt_search(&mut self) {
        if let Some(active) = &self.search {
            active.stop.store(true, Ordering::Relaxed);
        }
    }

    /// Interrupt and wait for the worker to exit.
    fn stop_search(&mut self) {
        if let Some(active) = self.search.take() {
            active.stop.store(true, Ordering::Relaxed);
            let _ = active.handle.join();
        }
    }

    /// Access to the current position, for tests.
    pub fn game(&self) -> &Game {
        &self.game
    }
}

impl<W: Write + Send + 'static> Drop for UciEngine<W> {
    fn drop(&mut self) {
        self.stop_search();
    }
}

/// How long to think with `remaining` milliseconds on the clock.
///
/// A flat share of the remaining time with a floor so the engine always has
/// something to report, and a cap so it never burns the whole clock on one move.
fn spend_budget(remaining_ms: u64, increment_ms: u64, moves_to_go: Option<u64>) -> u64 {
    let moves = moves_to_go.unwrap_or(30).max(1);
    let share = remaining_ms / moves + increment_ms / 2;
    share
        .clamp(50, 5_000)
        .min(remaining_ms.saturating_sub(20).max(20))
}

/// Convert a search score into UCI's `cp` / `mate` vocabulary.
pub fn score_description(score: i32) -> String {
    if score.abs() >= MATE_THRESHOLD {
        let plies = crate::search::MATE_SCORE - score.abs();
        let moves = (plies + 1) / 2;
        format!("mate {}", if score > 0 { moves } else { -moves })
    } else {
        format!("cp {score}")
    }
}

/// The starting position, exposed for the binary.
pub fn initial_game() -> Game {
    Game::from_position(Position::startpos())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// A `Write` that records everything, so tests can inspect the transcript.
    #[derive(Clone, Default)]
    struct Transcript(Arc<Mutex<Vec<u8>>>);

    impl Transcript {
        fn lines(&self) -> Vec<String> {
            let buffer = self.0.lock().unwrap();
            String::from_utf8(buffer.clone())
                .unwrap()
                .lines()
                .map(str::to_string)
                .collect()
        }
    }

    impl Write for Transcript {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    fn engine() -> (UciEngine<Transcript>, Transcript) {
        let transcript = Transcript::default();
        let engine = UciEngine::new(transcript.clone());
        (engine, transcript)
    }

    #[test]
    fn handshake() {
        let (mut engine, transcript) = engine();
        assert!(engine.handle_command("uci"));
        let lines = transcript.lines();
        assert!(lines.iter().any(|line| line.contains("id name Rookwood")));
        assert_eq!(lines.last().map(String::as_str), Some("uciok"));
    }

    #[test]
    fn isready_always_answers() {
        let (mut engine, transcript) = engine();
        engine.handle_command("isready");
        assert_eq!(transcript.lines(), vec!["readyok".to_string()]);
    }

    #[test]
    fn position_with_moves_is_applied() {
        let (mut engine, _transcript) = engine();
        engine.handle_command("position startpos moves e2e4 e7e5 g1f3");
        assert_eq!(engine.game().ply(), 3);
        assert!(
            engine
                .game()
                .fen()
                .starts_with("rnbqkbnr/pppp1ppp/8/4p3/4P3/5N2/PPPP1PPP/RNBQKB1R")
        );
    }

    #[test]
    fn position_fen_is_applied() {
        let (mut engine, _) = engine();
        engine.handle_command("position fen 8/8/8/8/8/8/8/K6k w - - 0 1");
        assert_eq!(engine.game().fen(), "8/8/8/8/8/8/8/K6k w - - 0 1");
    }

    #[test]
    fn perft_command_reports_the_published_count() {
        let (mut engine, transcript) = engine();
        engine.handle_command("position startpos");
        engine.handle_command("perft 3");
        assert!(
            transcript
                .lines()
                .iter()
                .any(|line| line.contains("perft 3 nodes 8902")),
            "unexpected output: {:?}",
            transcript.lines()
        );
    }

    #[test]
    fn go_depth_returns_a_bestmove() {
        let (mut engine, transcript) = engine();
        engine.handle_command("position startpos");
        engine.handle_command("go depth 2");
        engine.wait();
        let lines = transcript.lines();
        let bestmove = lines
            .iter()
            .find(|line| line.starts_with("bestmove"))
            .expect("bestmove line");
        let mut position = engine.game().clone();
        let text = bestmove.trim_start_matches("bestmove ");
        assert!(
            text != "0000",
            "engine reported no move in a legal position"
        );
        assert!(
            position.parse_uci(text).is_some(),
            "bestmove {text} is not legal"
        );
        assert!(lines.iter().any(|line| line.starts_with("info depth")));
    }

    #[test]
    fn illegal_move_is_reported_and_ignored() {
        let (mut engine, transcript) = engine();
        engine.handle_command("position startpos moves e2e5");
        assert_eq!(engine.game().ply(), 0);
        assert!(
            transcript
                .lines()
                .iter()
                .any(|line| line.contains("illegal move"))
        );
    }

    #[test]
    fn quit_stops_the_loop() {
        let (mut engine, _) = engine();
        assert!(!engine.handle_command("quit"));
    }

    #[test]
    fn clock_budget_is_sane() {
        assert_eq!(spend_budget(1_000, 0, None), 50);
        assert_eq!(spend_budget(60_000, 0, Some(30)), 2_000);
        assert!(spend_budget(100, 0, None) <= 100);
    }
}
