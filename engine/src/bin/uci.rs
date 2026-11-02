//! `rookwood-uci` — the engine as a UCI-speaking command line program.

use rookwood::uci::UciEngine;
use std::io;

fn main() -> io::Result<()> {
    let stdin = io::stdin();
    let mut engine = UciEngine::new(io::stdout());
    engine.run(stdin.lock())
}
