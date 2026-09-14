#[path = "../../engine/tests/support/schema.rs"]
#[allow(dead_code)]
mod schema;

use std::fs;
use std::path::PathBuf;

use proptest::prelude::*;
use serde_json::{Value, json};
use tiger_sparring::ledger::{
    EngineIdentity, EnvironmentIdentity, JsonFileSink, Matchup, Report, ReportError, ReportSink,
    SinkError,
};
use tiger_sparring::runner::GameResult;
use tiger_sparring::statistics::{StatisticsError, Tally, summarize};
use tiger_sparring::transcript::GameOutcome;

fn contract() -> Value {
    schema::load_contract("sparring-report.schema.json")
}

fn engine() -> EngineIdentity {
    EngineIdentity {
        name: "tiger-engine".to_owned(),
        commit: "0df5464a1b2c3d4e5f60718293a4b5c6d7e8f901".to_owned(),
    }
}

fn environment() -> EnvironmentIdentity {
    EnvironmentIdentity {
        cpu: "AMD Ryzen 7 5700X 8-Core Processor".to_owned(),
        kernel: "Linux 7.2.5-3-omarchy x86_64".to_owned(),
        rustc: "rustc 1.98.1".to_owned(),
        rules_cli_release: "v1.2.3".to_owned(),
    }
}

fn game(seed: u64, outcome: GameOutcome, turns: u32) -> GameResult {
    GameResult {
        seed,
        outcome,
        turns,
    }
}

/// Seeds 1..=n with the outcomes cycling through `pattern`; every game 40 turns.
fn results(pattern: &[GameOutcome], n: u64) -> Vec<GameResult> {
    (1..=n)
        .map(|seed| game(seed, pattern[(seed as usize - 1) % pattern.len()], 40))
        .collect()
}

fn a_matchup() -> Matchup {
    let results = vec![
        game(1, GameOutcome::Win, 48),
        game(2, GameOutcome::Loss, 22),
        game(3, GameOutcome::Win, 80),
        game(4, GameOutcome::Draw, 300),
    ];
    Matchup::assemble("tiger", "shapeshifter", &results).expect("a matchup")
}

fn a_report() -> Report {
    let second = Matchup::assemble(
        "baseline",
        "shapeshifter",
        &results(&[GameOutcome::Loss], 30),
    )
    .unwrap();
    Report::new(engine(), environment(), vec![a_matchup(), second]).expect("a report")
}

// ---- assembling a matchup -------------------------------------------------------------------

#[test]
fn a_matchup_tallies_the_results_and_keeps_the_seeds_in_order() {
    let matchup = a_matchup();

    assert_eq!(
        (matchup.challenger.as_str(), matchup.opponent.as_str()),
        ("tiger", "shapeshifter")
    );
    assert_eq!(matchup.seeds, [1, 2, 3, 4]);
    assert_eq!((matchup.wins, matchup.losses, matchup.draws), (2, 1, 1));
}

#[test]
fn the_rate_and_interval_come_from_the_statistics_module() {
    let matchup = a_matchup();

    let summary = summarize(&Tally {
        wins: 2,
        losses: 1,
        draws: 1,
    })
    .unwrap();

    assert!((matchup.win_rate - summary.win_rate).abs() < 1e-12);
    assert_eq!(matchup.wilson95, summary.wilson95);
    assert!(
        (matchup.win_rate - 0.5).abs() < 1e-12,
        "a draw is not a win"
    );
}

#[test]
fn the_mean_length_averages_every_game() {
    assert!((a_matchup().mean_turns - 112.5).abs() < 1e-12);
}

#[test]
fn a_matchup_with_no_games_is_a_statistics_error() {
    assert_eq!(
        Matchup::assemble("tiger", "base", &[]),
        Err(ReportError::Statistics(StatisticsError::EmptySample))
    );
}

// ---- the report and the schema ---------------------------------------------------------------------

#[test]
fn a_report_validates_against_the_sparring_schema_and_says_what_it_measured() {
    let json = a_report().to_json();

    assert_eq!(schema::validate(&contract(), &json), Ok(()), "{json:#}");
    assert_eq!(json["schema_version"], "1.0.0");
    assert_eq!(
        json["engine"]["commit"],
        "0df5464a1b2c3d4e5f60718293a4b5c6d7e8f901"
    );
    assert_eq!(json["environment"]["rules_cli_release"], "v1.2.3");
    let first = &json["matchups"][0];
    assert_eq!(first["seeds"], json!([1, 2, 3, 4]));
    assert_eq!(
        (
            first["wins"].as_u64(),
            first["losses"].as_u64(),
            first["draws"].as_u64()
        ),
        (Some(2), Some(1), Some(1))
    );
    assert_eq!(first["win_rate_wilson95"].as_array().map(Vec::len), Some(2));
    assert!((first["mean_turns"].as_f64().unwrap() - 112.5).abs() < 1e-12);
}

#[test]
fn a_report_needs_a_real_commit_and_at_least_one_matchup() {
    let short = EngineIdentity {
        commit: "abc123".to_owned(),
        ..engine()
    };

    assert_eq!(
        Report::new(short, environment(), vec![a_matchup()]),
        Err(ReportError::CommitTooShort)
    );
    assert_eq!(
        Report::new(engine(), environment(), Vec::new()),
        Err(ReportError::NoMatchups)
    );
    assert!(
        Report::new(
            EngineIdentity {
                commit: "abc1234".to_owned(),
                ..engine()
            },
            environment(),
            vec![a_matchup()]
        )
        .is_ok()
    );
}

#[test]
fn the_schema_check_rejects_every_kind_of_broken_report() {
    let good = a_report().to_json();
    assert_eq!(
        schema::validate(&contract(), &good),
        Ok(()),
        "the unbroken report must be valid"
    );
    let mut cases: Vec<(&str, Value)> = Vec::new();
    let mut edit = |name, change: &dyn Fn(&mut Value)| {
        let mut broken = good.clone();
        change(&mut broken);
        cases.push((name, broken));
    };
    edit("missing member", &|v| {
        v.as_object_mut().unwrap().remove("environment");
    });
    edit("extra member", &|v| v["notes"] = json!("free text"));
    edit("wrong version", &|v| v["schema_version"] = json!("2.0.0"));
    edit("short commit", &|v| v["engine"]["commit"] = json!("abc"));
    edit("no matchups", &|v| v["matchups"] = json!([]));
    edit("no seeds", &|v| v["matchups"][0]["seeds"] = json!([]));
    edit("three interval ends", &|v| {
        v["matchups"][0]["win_rate_wilson95"] = json!([0.1, 0.5, 0.9])
    });
    edit("rate above one", &|v| {
        v["matchups"][0]["win_rate"] = json!(1.5)
    });
    edit("negative wins", &|v| v["matchups"][0]["wins"] = json!(-1));

    for (name, broken) in cases {
        assert!(
            schema::validate(&contract(), &broken).is_err(),
            "{name} must be rejected"
        );
    }
}

proptest! {
    #[test]
    fn any_tally_of_results_makes_a_valid_matchup_that_adds_up(wins in 0u64..25, losses in 0u64..25, draws in 0u64..25) {
        prop_assume!(wins + losses + draws > 0);
        let mut outcomes = vec![GameOutcome::Win; wins as usize];
        outcomes.extend(vec![GameOutcome::Loss; losses as usize]);
        outcomes.extend(vec![GameOutcome::Draw; draws as usize]);
        let games: Vec<GameResult> = outcomes.iter().enumerate().map(|(i, &o)| game(i as u64 + 1, o, 10 + i as u32)).collect();

        let matchup = Matchup::assemble("tiger", "base", &games).unwrap();
        let report = Report::new(engine(), environment(), vec![matchup.clone()]).unwrap();

        prop_assert_eq!(u64::from(matchup.wins) + u64::from(matchup.losses) + u64::from(matchup.draws), games.len() as u64);
        prop_assert_eq!(matchup.seeds.len(), games.len());
        prop_assert_eq!(schema::validate(&contract(), &report.to_json()), Ok(()));
    }
}

// ---- the file sink ------------------------------------------------------------------------------------

struct Scratch(PathBuf);

impl Scratch {
    fn new(label: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("tiger-ledger-{}-{label}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn the_file_sink_writes_a_report_that_reads_back_and_validates() {
    let scratch = Scratch::new("write");
    let path = scratch.0.join("nested/dir/report.json");
    let report = a_report();

    JsonFileSink::new(path.clone(), false)
        .write(&report)
        .expect("written");

    let stored: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(stored, report.to_json());
    assert_eq!(schema::validate(&contract(), &stored), Ok(()));
}

#[test]
fn an_existing_report_is_never_overwritten_by_default() {
    let scratch = Scratch::new("refuse");
    let path = scratch.0.join("report.json");
    fs::write(&path, "the earlier report").unwrap();

    let error = JsonFileSink::new(path.clone(), false)
        .write(&a_report())
        .expect_err("must refuse");

    assert_eq!(error, SinkError::AlreadyExists(path.clone()));
    assert_eq!(fs::read_to_string(&path).unwrap(), "the earlier report");
}

#[test]
fn the_overwrite_flag_replaces_an_existing_report() {
    let scratch = Scratch::new("replace");
    let path = scratch.0.join("report.json");
    fs::write(&path, "the earlier report").unwrap();

    JsonFileSink::new(path.clone(), true)
        .write(&a_report())
        .expect("replaced");

    let stored: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(stored["schema_version"], "1.0.0");
}

#[test]
fn a_sink_is_usable_through_the_port() {
    let scratch = Scratch::new("port");
    let sink: Box<dyn ReportSink> = Box::new(JsonFileSink::new(scratch.0.join("r.json"), false));

    assert_eq!(sink.write(&a_report()), Ok(()));
}
