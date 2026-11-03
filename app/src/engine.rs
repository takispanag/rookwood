//! The search worker living behind the UI.
//!
//! Searching on the UI thread would freeze the window, so requests go to a
//! dedicated thread over a channel and results come back as they finish. A
//! shared atomic flag lets the UI abort a search instantly when the player
//! moves, undoes or starts a new game.

use rookwood::Game;
use rookwood::search::{SearchLimits, SearchResult, search};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::JoinHandle;

/// Why the engine was asked to think.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SearchKind {
    /// Fill the analysis panel; do not touch the board.
    Analyse,
    /// The engine is playing this move.
    Play,
}

/// A unit of work for the search thread.
pub struct SearchJob {
    /// Bumped whenever the board changes, so stale results can be dropped.
    pub generation: u64,
    pub game: Game,
    pub limits: SearchLimits,
    pub kind: SearchKind,
}

/// What came back.
pub struct SearchOutcome {
    pub generation: u64,
    pub kind: SearchKind,
    pub result: SearchResult,
}

/// Handle to the worker thread.
pub struct EngineHandle {
    jobs: Option<Sender<SearchJob>>,
    results: Receiver<SearchOutcome>,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
    busy: bool,
}

impl EngineHandle {
    pub fn spawn() -> EngineHandle {
        let (job_tx, job_rx) = mpsc::channel::<SearchJob>();
        let (result_tx, result_rx) = mpsc::channel::<SearchOutcome>();
        let stop = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&stop);

        let worker = std::thread::Builder::new()
            .name("rookwood-search".to_string())
            .spawn(move || {
                for job in job_rx {
                    flag.store(false, Ordering::Relaxed);
                    let mut game = job.game;
                    let result = search(&mut game, job.limits, Arc::clone(&flag));
                    let outcome = SearchOutcome {
                        generation: job.generation,
                        kind: job.kind,
                        result,
                    };
                    if result_tx.send(outcome).is_err() {
                        break;
                    }
                }
            })
            .expect("failed to start the search thread");

        EngineHandle {
            jobs: Some(job_tx),
            results: result_rx,
            stop,
            worker: Some(worker),
            busy: false,
        }
    }

    /// Queue a search. Returns false when one is already running.
    pub fn submit(&mut self, job: SearchJob) -> bool {
        if self.busy {
            return false;
        }
        self.stop.store(false, Ordering::Relaxed);
        self.busy = true;
        let sent = self
            .jobs
            .as_ref()
            .is_some_and(|jobs| jobs.send(job).is_ok());
        if !sent {
            self.busy = false;
        }
        sent
    }

    /// Take a finished search, if one is ready.
    pub fn poll(&mut self) -> Option<SearchOutcome> {
        match self.results.try_recv() {
            Ok(outcome) => {
                self.busy = false;
                Some(outcome)
            }
            Err(_) => None,
        }
    }

    /// Ask the worker to stop at its next cancellation check.
    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }

    pub fn is_busy(&self) -> bool {
        self.busy
    }
}

impl Drop for EngineHandle {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        // Closing the channel ends the worker's receive loop.
        self.jobs = None;
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
