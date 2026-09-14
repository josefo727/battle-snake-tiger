//! Orchestrating a whole benchmark: launch the servers, prove them alive, play
//! every seed for the challenger and for the baseline against each opponent, and
//! write the report.

use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;

use crate::ledger::{
    EngineIdentity, EnvironmentIdentity, Matchup, Report, ReportError, ReportSink, SinkError,
};
use crate::roster::{Entry, Roster};
use crate::runner::{Contestant, Duel, GameResult, RunnerError, SparringRunner};

/// A launched server: its URL, and a guard that stops it when dropped.
pub struct RunningServer {
    pub url: String,
    _guard: Box<dyn Send>,
}

impl RunningServer {
    #[must_use]
    pub fn new(url: String, guard: Box<dyn Send>) -> Self {
        Self { url, _guard: guard }
    }
}

/// Starts the servers of a roster.
pub trait Launcher {
    /// # Errors
    ///
    /// Fails when the server cannot be started or never becomes reachable.
    fn start(&self, entry: &Entry) -> Result<RunningServer, String>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plan {
    pub seeds: Vec<u64>,
    /// Games played at the same time (1 for strict one-at-a-time play).
    pub workers: usize,
}

#[derive(Debug, PartialEq)]
pub struct OpponentVerdict {
    pub opponent: String,
    pub challenger_win_rate: f64,
    pub baseline_win_rate: f64,
    /// Acceptance criterion 8: strictly greater than the baseline's rate.
    pub beats_baseline: bool,
}

#[derive(Debug, PartialEq)]
pub struct BenchmarkOutcome {
    pub matchups: Vec<Matchup>,
    pub verdicts: Vec<OpponentVerdict>,
}

impl BenchmarkOutcome {
    /// Whether the challenger beat the baseline against every opponent.
    #[must_use]
    pub fn criterion_met(&self) -> bool {
        !self.verdicts.is_empty() && self.verdicts.iter().all(|v| v.beats_baseline)
    }
}

#[derive(Debug, PartialEq)]
pub enum BenchmarkError {
    Launch { id: String, reason: String },
    Runner(RunnerError),
    Report(ReportError),
    Sink(SinkError),
}

/// Runs the benchmark described by `roster` and `plan` and writes the report.
///
/// # Errors
///
/// Fails before any game when a server cannot be launched or is not alive, and
/// otherwise at the first failed game or unwritable report.
pub fn run_benchmark<R, L, S>(
    roster: &Roster,
    plan: &Plan,
    runner: &R,
    launcher: &L,
    sink: &S,
    engine: EngineIdentity,
    environment: EnvironmentIdentity,
) -> Result<BenchmarkOutcome, BenchmarkError>
where
    R: SparringRunner + Sync,
    L: Launcher,
    S: ReportSink,
{
    // Servers are stopped when `servers` drops, on success and on every early return.
    let mut servers: Vec<Contestant> = Vec::new();
    let mut guards = Vec::new();
    for entry in [&roster.challenger, &roster.baseline]
        .into_iter()
        .chain(&roster.opponents)
    {
        let server = launcher
            .start(entry)
            .map_err(|reason| BenchmarkError::Launch {
                id: entry.id.clone(),
                reason,
            })?;
        servers.push(Contestant {
            name: entry.id.clone(),
            url: server.url.clone(),
        });
        guards.push(server);
    }
    for contestant in &servers {
        runner
            .check_alive(contestant)
            .map_err(BenchmarkError::Runner)?;
    }

    let (challenger, baseline) = (&servers[0], &servers[1]);
    let mut matchups = Vec::new();
    for opponent in &servers[2..] {
        for us in [challenger, baseline] {
            let results = play_seeds(runner, us, opponent, plan)?;
            matchups.push(
                Matchup::assemble(&us.name, &opponent.name, &results)
                    .map_err(BenchmarkError::Report)?,
            );
        }
    }
    let report =
        Report::new(engine, environment, matchups.clone()).map_err(BenchmarkError::Report)?;
    sink.write(&report).map_err(BenchmarkError::Sink)?;

    let verdicts = matchups
        .chunks(2)
        .map(|pair| OpponentVerdict {
            opponent: pair[0].opponent.clone(),
            challenger_win_rate: pair[0].win_rate,
            baseline_win_rate: pair[1].win_rate,
            beats_baseline: pair[0].win_rate > pair[1].win_rate,
        })
        .collect();
    Ok(BenchmarkOutcome { matchups, verdicts })
}

/// Plays every seed of `plan` for one pairing, `plan.workers` games at a time, and
/// returns the results in seed order; a failed game is reported by the lowest seed.
fn play_seeds<R: SparringRunner + Sync>(
    runner: &R,
    challenger: &Contestant,
    opponent: &Contestant,
    plan: &Plan,
) -> Result<Vec<GameResult>, BenchmarkError> {
    let play = |seed: u64| {
        runner.play(&Duel {
            challenger: challenger.clone(),
            opponent: opponent.clone(),
            seed,
        })
    };
    if plan.workers <= 1 {
        return plan
            .seeds
            .iter()
            .map(|&seed| play(seed).map_err(BenchmarkError::Runner))
            .collect();
    }
    let next = AtomicUsize::new(0);
    let outcomes: Mutex<Vec<(usize, Result<GameResult, RunnerError>)>> = Mutex::new(Vec::new());
    thread::scope(|scope| {
        for _ in 0..plan.workers {
            scope.spawn(|| {
                loop {
                    let index = next.fetch_add(1, Ordering::SeqCst);
                    let Some(&seed) = plan.seeds.get(index) else {
                        break;
                    };
                    let result = play(seed);
                    outcomes
                        .lock()
                        .expect("never poisoned")
                        .push((index, result));
                }
            });
        }
    });
    let mut outcomes = outcomes.into_inner().expect("never poisoned");
    outcomes.sort_by_key(|(index, _)| *index);
    outcomes
        .into_iter()
        .map(|(_, result)| result.map_err(BenchmarkError::Runner))
        .collect()
}
