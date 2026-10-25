# Rookwood

A chess engine and desktop analysis board written from scratch in Rust.

Every part of the chess logic is implemented in this repository: the board
representation, attack generation, move legality, FEN, the draw rules,
evaluation, alpha-beta search and the UCI protocol. There are no chess
libraries, no copied engine code and no image or sound assets — even the pieces
are drawn with vector shapes.

```
┌───────────────────────────────┐
│  rookwood (engine crate)      │   zero dependencies
│  rules · eval · search · UCI  │
└───────────┬───────────────────┘
            │
     ┌──────┴───────┐
     │              │
┌────▼─────┐  ┌─────▼──────────┐
│ rookwood │  │ rookwood-gui   │
│ -uci     │  │ eframe + egui  │
└──────────┘  └────────────────┘
```

## Features

**Rules** — full legal move generation with castling, en passant, and all four
promotions; check, checkmate and stalemate detection; fifty-move rule;
threefold repetition; insufficient material. Positions are read and written as
FEN (all six fields).

**Engine** — negamax alpha-beta with a capture-only quiescence search, MVV-LVA
move ordering, principal-variation tracking, and an incremental Zobrist key for
repetition detection and draw scoring.

**Interfaces** — a UCI front end suitable for any chess GUI, and a native
analysis board with drag-and-drop, legal-move hints, an evaluation bar, a move
list, a promotion picker and an engine that plays either side.

**Validation** — `perft` through the standard test positions, plus unit tests
for every rule that is easy to get subtly wrong.

## Quick start

Requires Rust 1.95 or newer.

```bash
# Desktop board
cargo run --release -p rookwood-gui

# UCI engine (for Arena, CuteChess, En Croissant, ...)
cargo run --release -p rookwood --bin rookwood-uci
```

The release binary is at `target/release/rookwood-uci` (`.exe` on Windows).

### Using it with a chess GUI

Add `rookwood-uci` as a UCI engine, then hit "new game". A short session from
the command line looks like this:

```
uci
id name Rookwood 0.1.0
id author Takis Panagiotopoulos
uciok
position startpos moves e2e4 e7e5
go depth 6
info depth 6 score cp 31 nodes 4211330 nps 14829000 time 284 pv g1f3 b8c6 f1b5
bestmove g1f3
```

## UCI commands

| Command | Notes |
|---|---|
| `uci`, `isready` | handshake |
| `position startpos \| fen ... [moves ...]` | sets the board, replays the moves |
| `go depth N` | fixed depth |
| `go movetime MS` | wall-clock budget |
| `go wtime/btime/winc/binc/movestogo` | derives a budget from the clock |
| `go infinite`, `stop` | search until interrupted |
| `go nodes N` | hard node budget |
| `perft N` | node count with timing, for testing |
| `d`, `fen`, `eval` | debug output (board, FEN, static score) |
| `ucinewgame`, `quit` | housekeeping |

## Perft

Move generation is checked against the published node counts. All of these
match exactly; run them yourself with `perft N`:

| Position | Depth | Nodes | Time |
|---|---|---:|---:|
| Start position | 5 | 4,865,609 | 91 ms |
| Kiwipete | 4 | 4,085,603 | 88 ms |
| Position 3 (`8/2p5/3p4/KP5r/...`) | 4 | 43,238 | 1 ms |
| Position 4 (`r3k2r/Pppp1ppp/...`) | 4 | 422,333 | 7 ms |
| Position 5 (`rnbq1k1r/pp1Pbppp/...`) | 4 | 2,103,487 | 41 ms |
| Position 6 (`r4rk1/1pp1qppp/...`) | 4 | 3,894,594 | 67 ms |

The shallow depths run as part of `cargo test`; the deeper ones are in the UCI
`perft` command so they can be re-run without a rebuild.

## Testing

```bash
cargo test --workspace
```

The suite covers square and bitboard arithmetic, attack-table correctness
against a reference ray walk, FEN round-trips and rejection of malformed input,
castling rights bookkeeping, en passant, promotions, pins, key/undo
consistency, perft, draw rules, evaluation symmetry, search tactics (mates,
hanging material, PV legality) and UCI transcripts driven through in-memory
streams.

## How it works

| Module | Responsibility |
|---|---|
| `types` | colours, piece kinds, squares |
| `bitboard` | 64-bit square sets |
| `attacks` | leaper tables and ray-based sliding attacks |
| `moves` | 16-bit move encoding, fixed-capacity move lists |
| `position` | board state, FEN, make/unmake, key updates |
| `movegen` | pseudo-legal generation, then a make/unmake legality filter |
| `game` | move history, repetition counting, draw rules |
| `eval` | material and piece-square tables |
| `search` | alpha-beta, quiescence, principal variation |
| `perft` | move generator validation |
| `uci` | command loop, worker thread, clock budgets |

A few decisions worth calling out:

- **Squares are `rank * 8 + file`**, the same numbering as the bitboard, so the
  two representations convert for free.
- **Sliding attacks use ray masks, not magic numbers.** For each square and
  direction the engine stores the squares beyond it, then clips the ray at the
  first blocker with one `trailing_zeros`/`leading_zeros`. That is a handful of
  lines instead of a generator and a table of magic constants, and it still
  reaches roughly 50 million nodes per second on perft.
- **Legality is filtered by making the move**, not by computing pins and
  discovered checks. It costs some speed and removes a whole class of bugs; the
  perft table above is the evidence that the trade is sound.
- **The search runs on a worker thread** and is cancelled through an atomic
  flag, so `stop` (and the GUI's Stop button) is answered within a few
  milliseconds.

## Performance

Measured on one core of a desktop CPU, release build:

- `perft 5` from the start position: ~53M nodes/s
- search at depth 6 from the start position: ~14.6M nodes/s

## Roadmap

- Transposition table, iterative deepening and a proper time manager
- Better move ordering (killers, history, SEE) and null-move pruning
- SAN and PGN import/export
- Richer evaluation: pawn structure, mobility, king safety, passed pawns

## License

MIT — see [LICENSE](LICENSE).

Rookwood is an original implementation. No third-party chess code, evaluation
data or artwork is included.
