//! Orchestrating a whole benchmark: launch the servers, prove them alive, play
//! every seed for the challenger and for the baseline against each opponent, and
//! write the report.

use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;

use crate::ledger::{
    EngineIdentity, EnvironmentIdentity, Matchup, MeleeGameRecord, MeleeReport, MeleeReportSink,
    Report, ReportError, ReportSink, SinkError,
};
use crate::roster::{Entry, Roster};
use crate::runner::{
    Bout, BoutResult, Contestant, Duel, GameResult, MeleeRunner, RunnerError, SparringRunner,
};

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
    // Servers are stopped when the guards drop, on success and on every early return.
    let (servers, _guards) = launch_all(roster, runner, launcher)?;
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

/// The placement benchmark's outcome: the report written, and whether criterion 6 held.
#[derive(Debug, PartialEq)]
pub struct MeleeOutcome {
    pub report: MeleeReport,
}

impl MeleeOutcome {
    #[must_use]
    pub const fn criterion_met(&self) -> bool {
        self.report.summary.criterion_met
    }
}

/// Runs the four-snake placement benchmark of `roster` (its opponents are the
/// other seats, the first being the reference) and writes the report: every
/// seed with the challenger in seat one, then every seed with the baseline there.
///
/// # Errors
///
/// Fails like [`run_benchmark`].
pub fn run_melee_benchmark<R, L, S>(
    roster: &Roster,
    plan: &Plan,
    runner: &R,
    launcher: &L,
    sink: &S,
    engine: EngineIdentity,
    environment: EnvironmentIdentity,
) -> Result<MeleeOutcome, BenchmarkError>
where
    R: SparringRunner + MeleeRunner + Sync,
    L: Launcher,
    S: MeleeReportSink,
{
    let (servers, _guards) = launch_all(roster, runner, launcher)?;
    let (challenger, baseline, opponents) = (&servers[0], &servers[1], &servers[2..]);
    let mut games = Vec::new();
    for seat_one in [challenger, baseline] {
        let seats: Vec<Contestant> = std::iter::once(seat_one.clone())
            .chain(opponents.iter().cloned())
            .collect();
        let names: Vec<String> = seats.iter().map(|s| s.name.clone()).collect();
        let results = play_bouts(runner, &seats, plan)?;
        games.extend(
            results
                .iter()
                .map(|result| MeleeGameRecord::from_bout(&names, result)),
        );
    }
    let opponent_ids: Vec<String> = opponents.iter().map(|o| o.name.clone()).collect();
    let report = MeleeReport::assemble(
        engine,
        environment,
        &challenger.name,
        &baseline.name,
        &opponent_ids,
        games,
    )
    .map_err(BenchmarkError::Report)?;
    sink.write_melee(&report).map_err(BenchmarkError::Sink)?;
    Ok(MeleeOutcome { report })
}

/// Starts every roster entry (challenger, baseline, opponents in order) and
/// proves each alive; the guards stop the servers when dropped.
fn launch_all<R: SparringRunner, L: Launcher>(
    roster: &Roster,
    runner: &R,
    launcher: &L,
) -> Result<(Vec<Contestant>, Vec<RunningServer>), BenchmarkError> {
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
    Ok((servers, guards))
}

/// Plays every seed of `plan` with `seats`, `plan.workers` games at a time, in
/// seed order.
fn play_bouts<R: MeleeRunner + Sync>(
    runner: &R,
    seats: &[Contestant],
    plan: &Plan,
) -> Result<Vec<BoutResult>, BenchmarkError> {
    play_in_order(plan, |seed| {
        runner.play_bout(&Bout {
            seats: seats.to_vec(),
            seed,
        })
    })
}

/// Plays every seed of `plan` for one pairing, `plan.workers` games at a time, and
/// returns the results in seed order; a failed game is reported by the lowest seed.
fn play_seeds<R: SparringRunner + Sync>(
    runner: &R,
    challenger: &Contestant,
    opponent: &Contestant,
    plan: &Plan,
) -> Result<Vec<GameResult>, BenchmarkError> {
    play_in_order(plan, |seed| {
        runner.play(&Duel {
            challenger: challenger.clone(),
            opponent: opponent.clone(),
            seed,
        })
    })
}

/// Runs `play` for every seed of `plan`, `plan.workers` at a time, and returns
/// the results in seed order; a failed game is reported by the lowest seed.
fn play_in_order<T: Send>(
    plan: &Plan,
    play: impl Fn(u64) -> Result<T, RunnerError> + Sync,
) -> Result<Vec<T>, BenchmarkError> {
    if plan.workers <= 1 {
        return plan
            .seeds
            .iter()
            .map(|&seed| play(seed).map_err(BenchmarkError::Runner))
            .collect();
    }
    let next = AtomicUsize::new(0);
    let outcomes: Mutex<Vec<(usize, Result<T, RunnerError>)>> = Mutex::new(Vec::new());
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
