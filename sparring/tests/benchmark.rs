#[path = "../../engine/tests/support/schema.rs"]
#[allow(dead_code)]
mod schema;

use std::sync::Mutex;

use serde_json::Value;
use tiger_sparring::benchmark::{BenchmarkError, Launcher, Plan, RunningServer, run_benchmark};
use tiger_sparring::ledger::{EngineIdentity, EnvironmentIdentity, Report, ReportSink, SinkError};
use tiger_sparring::roster::{Entry, Launch, Roster};
use tiger_sparring::runner::{Contestant, Duel, GameResult, RunnerError, SparringRunner};
use tiger_sparring::transcript::GameOutcome;

fn entry(id: &str) -> Entry {
    Entry {
        id: id.to_owned(),
        launch: Launch {
            program: format!("/bin/{id}"),
            args: Vec::new(),
            env: Default::default(),
        },
    }
}

fn roster() -> Roster {
    Roster {
        rules_cli_release: "v1.2.3".to_owned(),
        challenger: entry("tiger"),
        baseline: entry("baseline"),
        opponents: vec![entry("shapeshifter"), entry("flood")],
    }
}

fn engine() -> EngineIdentity {
    EngineIdentity {
        name: "tiger-engine".to_owned(),
        commit: "0123456789abcdef".to_owned(),
    }
}

fn environment() -> EnvironmentIdentity {
    EnvironmentIdentity {
        cpu: "cpu".to_owned(),
        kernel: "kernel".to_owned(),
        rustc: "rustc".to_owned(),
        rules_cli_release: "v1.2.3".to_owned(),
    }
}

fn plan(workers: usize) -> Plan {
    Plan {
        seeds: (1..=30).collect(),
        workers,
    }
}

// ---- doubles ----------------------------------------------------------------------------------

/// Launches nothing: the "server" is a URL naming the entry, and a live count shows
/// that every guard was dropped.
struct FakeLauncher {
    live: std::sync::Arc<Mutex<i32>>,
    refuse: Option<&'static str>,
}

struct Guard(std::sync::Arc<Mutex<i32>>);

impl Drop for Guard {
    fn drop(&mut self) {
        *self.0.lock().unwrap() -= 1;
    }
}

impl FakeLauncher {
    fn new() -> Self {
        Self {
            live: std::sync::Arc::default(),
            refuse: None,
        }
    }
}

impl Launcher for FakeLauncher {
    fn start(&self, entry: &Entry) -> Result<RunningServer, String> {
        if self.refuse == Some(entry.id.as_str()) {
            return Err("never became reachable".to_owned());
        }
        *self.live.lock().unwrap() += 1;
        Ok(RunningServer::new(
            format!("http://{}", entry.id),
            Box::new(Guard(self.live.clone())),
        ))
    }
}

/// Plays deterministic games: the challenger named `winner` takes the seeds not
/// divisible by `losing_modulus`; anything else loses those, and every third-side
/// seed divisible by 10 is a draw.
struct FakeRunner {
    calls: Mutex<Vec<String>>,
    tiger_wins_out_of_30: u64,
    baseline_wins_out_of_30: u64,
    dead: Option<&'static str>,
}

impl FakeRunner {
    fn new(tiger: u64, baseline: u64) -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
            tiger_wins_out_of_30: tiger,
            baseline_wins_out_of_30: baseline,
            dead: None,
        }
    }
    fn played(&self) -> Vec<String> {
        self.calls
            .lock()
            .unwrap()
            .iter()
            .filter(|c| c.starts_with("play"))
            .cloned()
            .collect()
    }
}

impl SparringRunner for FakeRunner {
    fn check_alive(&self, contestant: &Contestant) -> Result<(), RunnerError> {
        self.calls
            .lock()
            .unwrap()
            .push(format!("alive {}", contestant.name));
        if self.dead == Some(contestant.name.as_str()) {
            return Err(RunnerError::Unreachable {
                name: contestant.name.clone(),
                url: contestant.url.clone(),
                reason: "refused".to_owned(),
            });
        }
        Ok(())
    }

    fn play(&self, duel: &Duel) -> Result<GameResult, RunnerError> {
        self.calls.lock().unwrap().push(format!(
            "play {} {} {}",
            duel.challenger.name, duel.opponent.name, duel.seed
        ));
        let wins = if duel.challenger.name == "tiger" {
            self.tiger_wins_out_of_30
        } else {
            self.baseline_wins_out_of_30
        };
        let outcome = if duel.seed <= wins {
            GameOutcome::Win
        } else {
            GameOutcome::Loss
        };
        Ok(GameResult {
            seed: duel.seed,
            outcome,
            turns: 50,
        })
    }
}

#[derive(Default)]
struct MemorySink {
    reports: Mutex<Vec<Value>>,
    refuse: bool,
}

impl ReportSink for MemorySink {
    fn write(&self, report: &Report) -> Result<(), SinkError> {
        if self.refuse {
            return Err(SinkError::AlreadyExists("report.json".into()));
        }
        self.reports.lock().unwrap().push(report.to_json());
        Ok(())
    }
}

fn run(
    runner: &FakeRunner,
    launcher: &FakeLauncher,
    sink: &MemorySink,
    workers: usize,
) -> Result<tiger_sparring::benchmark::BenchmarkOutcome, BenchmarkError> {
    run_benchmark(
        &roster(),
        &plan(workers),
        runner,
        launcher,
        sink,
        engine(),
        environment(),
    )
}

// ---- the benchmark ------------------------------------------------------------------------------

#[test]
fn thirty_games_are_played_for_each_challenger_against_each_opponent() {
    let runner = FakeRunner::new(20, 10);
    let outcome =
        run(&runner, &FakeLauncher::new(), &MemorySink::default(), 1).expect("a benchmark");

    let names: Vec<(&str, &str)> = outcome
        .matchups
        .iter()
        .map(|m| (m.challenger.as_str(), m.opponent.as_str()))
        .collect();
    assert_eq!(
        names,
        [
            ("tiger", "shapeshifter"),
            ("baseline", "shapeshifter"),
            ("tiger", "flood"),
            ("baseline", "flood")
        ]
    );
    for matchup in &outcome.matchups {
        assert_eq!(
            matchup.seeds,
            (1..=30).collect::<Vec<u64>>(),
            "{}",
            matchup.challenger
        );
        assert_eq!(matchup.wins + matchup.losses + matchup.draws, 30);
    }
    assert_eq!(runner.played().len(), 120);
}

#[test]
fn every_server_is_proved_alive_before_the_first_game() {
    let runner = FakeRunner::new(20, 10);
    run(&runner, &FakeLauncher::new(), &MemorySink::default(), 1).unwrap();

    let calls = runner.calls.lock().unwrap();
    let first_game = calls
        .iter()
        .position(|c| c.starts_with("play"))
        .expect("games were played");
    let alive: Vec<&str> = calls[..first_game].iter().map(String::as_str).collect();
    for name in ["tiger", "baseline", "shapeshifter", "flood"] {
        assert!(
            alive.contains(&format!("alive {name}").as_str()),
            "{name}: {alive:?}"
        );
    }
}

#[test]
fn an_unreachable_server_stops_everything_before_any_game() {
    let runner = FakeRunner {
        dead: Some("flood"),
        ..FakeRunner::new(20, 10)
    };

    let error =
        run(&runner, &FakeLauncher::new(), &MemorySink::default(), 1).expect_err("flood is dead");

    assert!(
        matches!(&error, BenchmarkError::Runner(RunnerError::Unreachable { name, .. }) if name == "flood"),
        "{error:?}"
    );
    assert!(runner.played().is_empty(), "no game may start");
}

#[test]
fn a_server_that_cannot_be_launched_fails_the_benchmark_naming_it() {
    let launcher = FakeLauncher {
        refuse: Some("shapeshifter"),
        ..FakeLauncher::new()
    };
    let runner = FakeRunner::new(20, 10);

    let error = run(&runner, &launcher, &MemorySink::default(), 1).expect_err("cannot launch");

    assert!(
        matches!(&error, BenchmarkError::Launch { id, .. } if id == "shapeshifter"),
        "{error:?}"
    );
    assert!(runner.played().is_empty());
    assert_eq!(
        *launcher.live.lock().unwrap(),
        0,
        "servers started earlier are stopped again"
    );
}

#[test]
fn the_report_is_written_once_and_validates_against_the_sparring_schema() {
    let sink = MemorySink::default();
    run(&FakeRunner::new(20, 10), &FakeLauncher::new(), &sink, 1).unwrap();

    let reports = sink.reports.lock().unwrap();
    assert_eq!(reports.len(), 1);
    let contract = schema::load_contract("sparring-report.schema.json");
    assert_eq!(schema::validate(&contract, &reports[0]), Ok(()));
    assert_eq!(reports[0]["matchups"].as_array().map(Vec::len), Some(4));
    assert_eq!(reports[0]["engine"]["commit"], "0123456789abcdef");
}

#[test]
fn a_report_the_sink_refuses_is_an_error_after_the_games() {
    let sink = MemorySink {
        refuse: true,
        ..MemorySink::default()
    };

    let error = run(&FakeRunner::new(20, 10), &FakeLauncher::new(), &sink, 1).expect_err("refused");

    assert!(
        matches!(error, BenchmarkError::Sink(SinkError::AlreadyExists(_))),
        "{error:?}"
    );
}

#[test]
fn the_verdict_needs_the_challenger_to_beat_the_baseline_strictly_against_each_opponent() {
    let better = run(
        &FakeRunner::new(20, 10),
        &FakeLauncher::new(),
        &MemorySink::default(),
        1,
    )
    .unwrap();
    let equal = run(
        &FakeRunner::new(15, 15),
        &FakeLauncher::new(),
        &MemorySink::default(),
        1,
    )
    .unwrap();
    let worse = run(
        &FakeRunner::new(5, 15),
        &FakeLauncher::new(),
        &MemorySink::default(),
        1,
    )
    .unwrap();

    assert!(better.criterion_met());
    assert_eq!(better.verdicts.len(), 2);
    assert!((better.verdicts[0].challenger_win_rate - 20.0 / 30.0).abs() < 1e-12);
    assert!((better.verdicts[0].baseline_win_rate - 10.0 / 30.0).abs() < 1e-12);
    assert!(!equal.criterion_met(), "a tie is not a win");
    assert!(!worse.criterion_met());
}

#[test]
fn parallel_workers_play_every_seed_exactly_once_and_keep_the_results_in_seed_order() {
    let runner = FakeRunner::new(20, 10);
    let outcome = run(&runner, &FakeLauncher::new(), &MemorySink::default(), 4).unwrap();

    for matchup in &outcome.matchups {
        assert_eq!(matchup.seeds, (1..=30).collect::<Vec<u64>>());
    }
    let mut played = runner.played();
    played.sort();
    played.dedup();
    assert_eq!(played.len(), 120, "no game twice");
}

#[test]
fn every_launched_server_is_stopped_when_the_benchmark_ends() {
    let launcher = FakeLauncher::new();

    run(
        &FakeRunner::new(20, 10),
        &launcher,
        &MemorySink::default(),
        2,
    )
    .unwrap();

    assert_eq!(*launcher.live.lock().unwrap(), 0);
}
