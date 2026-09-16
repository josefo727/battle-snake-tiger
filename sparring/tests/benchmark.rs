#[path = "../../engine/tests/support/schema.rs"]
#[allow(dead_code)]
mod schema;

use std::sync::Mutex;

use serde_json::Value;
use tiger_sparring::benchmark::{
    BenchmarkError, Launcher, Plan, RunningServer, run_benchmark, run_melee_benchmark,
};
use tiger_sparring::ledger::{
    EngineIdentity, EnvironmentIdentity, MeleeReport, MeleeReportSink, Report, ReportSink,
    SinkError,
};
use tiger_sparring::roster::{Entry, Launch, Roster};
use tiger_sparring::runner::{
    Bout, BoutResult, Contestant, Duel, GameResult, MeleeRunner, RunnerError, SparringRunner,
};
use tiger_sparring::transcript::GameOutcome;

fn entry(id: &str) -> Entry {
    Entry {
        id: id.to_owned(),
        launch: Launch {
            program: format!("/bin/{id}"),
            args: Vec::new(),
            env: Default::default(),
            stop: None,
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

// ---- the placement benchmark ---------------------------------------------------------------------

fn melee_roster() -> Roster {
    Roster {
        rules_cli_release: "v1.2.3".to_owned(),
        challenger: entry("tiger"),
        baseline: entry("baseline"),
        opponents: vec![entry("sanson"), entry("flood-a"), entry("flood-b")],
    }
}

/// Plays canned four-snake games: with the tiger in seat one, the tiger comes
/// 1st on odd seeds and 2nd on even ones (Sansón the other way round); with the
/// baseline in seat one, the baseline comes 4th and Sansón 1st; the Floods take
/// the rest and share a place on every third seed.
struct FakeMeleeRunner {
    calls: Mutex<Vec<String>>,
}

impl SparringRunner for FakeMeleeRunner {
    fn check_alive(&self, contestant: &Contestant) -> Result<(), RunnerError> {
        self.calls
            .lock()
            .unwrap()
            .push(format!("alive {}", contestant.name));
        Ok(())
    }

    fn play(&self, _duel: &Duel) -> Result<GameResult, RunnerError> {
        panic!("the placement benchmark plays bouts, not duels")
    }
}

impl MeleeRunner for FakeMeleeRunner {
    fn play_bout(&self, bout: &Bout) -> Result<BoutResult, RunnerError> {
        let names: Vec<&str> = bout.seats.iter().map(|s| s.name.as_str()).collect();
        self.calls
            .lock()
            .unwrap()
            .push(format!("bout {} {}", names.join(","), bout.seed));
        assert_eq!(&names[1..], ["sanson", "flood-a", "flood-b"]);
        let (first, second) = if bout.seed % 2 == 1 {
            (1.0, 2.0)
        } else {
            (2.0, 1.0)
        };
        let floods = if bout.seed.is_multiple_of(3) {
            (3.5, 3.5)
        } else {
            (3.0, 4.0)
        };
        let placements = if names[0] == "tiger" {
            vec![first, second, floods.0, floods.1]
        } else {
            vec![4.0, 1.0, 2.0, 3.0]
        };
        Ok(BoutResult {
            seed: bout.seed,
            placements,
            turns: 100,
        })
    }
}

#[derive(Default)]
struct MeleeMemorySink {
    reports: Mutex<Vec<Value>>,
}

impl MeleeReportSink for MeleeMemorySink {
    fn write_melee(&self, report: &MeleeReport) -> Result<(), SinkError> {
        self.reports.lock().unwrap().push(report.to_json());
        Ok(())
    }
}

#[test]
fn the_placement_benchmark_plays_every_seed_in_both_seatings_and_reports_the_means() {
    let runner = FakeMeleeRunner {
        calls: Mutex::new(Vec::new()),
    };
    let launcher = FakeLauncher::new();
    let sink = MeleeMemorySink::default();

    let outcome = run_melee_benchmark(
        &melee_roster(),
        &plan(1),
        &runner,
        &launcher,
        &sink,
        engine(),
        environment(),
    )
    .expect("the benchmark runs");

    let summary = &outcome.report.summary;
    assert_eq!(outcome.report.games.len(), 60);
    assert_eq!(summary.challenger, "tiger");
    assert_eq!(summary.baseline, "baseline");
    assert_eq!(summary.reference_opponent, "sanson");
    assert!((summary.challenger_mean - 1.5).abs() < 1e-9, "{summary:?}");
    assert!((summary.baseline_mean - 4.0).abs() < 1e-9, "{summary:?}");
    assert!((summary.reference_mean_in_challenger_games - 1.5).abs() < 1e-9);
    assert_eq!(summary.opponents.len(), 3);
    assert!((summary.opponents[0].mean_with_baseline - 1.0).abs() < 1e-9);
    assert!(summary.criterion_met, "1.5 < 4.0 and 1.5 <= 1.5");
    let calls = runner.calls.lock().unwrap().clone();
    assert_eq!(calls.iter().filter(|c| c.starts_with("alive")).count(), 5);
    assert_eq!(
        calls
            .iter()
            .filter(|c| c.starts_with("bout tiger,"))
            .count(),
        30
    );
    assert_eq!(
        calls
            .iter()
            .filter(|c| c.starts_with("bout baseline,"))
            .count(),
        30
    );
    assert_eq!(sink.reports.lock().unwrap().len(), 1);
    assert_eq!(
        *launcher.live.lock().unwrap(),
        0,
        "every server was stopped"
    );
}

#[test]
fn the_criterion_fails_when_the_challenger_places_worse_than_the_reference_or_not_better_than_the_baseline()
 {
    use tiger_sparring::ledger::MeleeGameRecord;
    let game = |seat_one: &str, seed, places: [f64; 4]| MeleeGameRecord {
        seat_one: seat_one.to_owned(),
        seed,
        turns: 10,
        placements: ["tiger", "sanson", "flood-a", "flood-b"]
            .iter()
            .map(|s| {
                if *s == "tiger" {
                    seat_one.to_owned()
                } else {
                    (*s).to_owned()
                }
            })
            .zip(places)
            .collect(),
    };
    let opponents = ["sanson", "flood-a", "flood-b"].map(str::to_owned);
    let assemble = |games| {
        MeleeReport::assemble(
            engine(),
            environment(),
            "tiger",
            "baseline",
            &opponents,
            games,
        )
        .expect("assembles")
    };

    // Worse than Sansón in the same games: tiger 2nd, Sansón 1st.
    let worse = assemble(vec![
        game("tiger", 1, [2.0, 1.0, 3.0, 4.0]),
        game("baseline", 1, [4.0, 1.0, 2.0, 3.0]),
    ]);
    assert!(!worse.summary.criterion_met);
    // Not better than the baseline: both 1st.
    let equal = assemble(vec![
        game("tiger", 1, [1.0, 2.0, 3.0, 4.0]),
        game("baseline", 1, [1.0, 2.0, 3.0, 4.0]),
    ]);
    assert!(!equal.summary.criterion_met);
    // Equal to Sansón's mean and better than the baseline: met.
    let met = assemble(vec![
        game("tiger", 1, [1.0, 2.0, 3.0, 4.0]),
        game("tiger", 2, [2.0, 1.0, 3.0, 4.0]),
        game("baseline", 1, [3.0, 1.0, 2.0, 4.0]),
        game("baseline", 2, [3.0, 1.0, 2.0, 4.0]),
    ]);
    assert!(met.summary.criterion_met);
    assert!((met.summary.opponents[1].mean_with_challenger - 3.0).abs() < 1e-9);
}
