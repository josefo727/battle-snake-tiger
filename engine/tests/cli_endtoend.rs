//! Plays the compiled engine through the official `battlesnake` rules CLI
//! (spec criteria 1 and 5): duel games against the sibling one-turn baseline, and a
//! three-snake game that must use the safety fallback. The CLI announces
//! `ruleset.version: "cli"`; the sibling's classifier once missed that and served
//! every local game from its fallback, so this suite checks the engine's own
//! decision log, not just that games finish.
//!
//! The games are `#[ignore]`d and driven by `scripts/run-endtoend`; the fast tests
//! pin the harness (digest check, transcript and log parsing, routing checks).

mod support;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

/// The `battlesnake` executable of the official rules release v1.2.3, by digest
/// (the archive's own digest is checked by `scripts/fetch-rules-oracle`).
const PINNED_EXECUTABLE_SHA256: &str =
    "0cc547966887fd49acc642548ea0ef0a9256b58a8652aa33231a862bb3b8b275";
#[allow(
    dead_code,
    reason = "scaffold: the heavy games use it in the green step"
)]
const OUR_NAME: &str = "tiger";
const DUEL_SHARE_FLOOR_PERCENT: f64 = 95.0;

// ---- the oracle and the transcript --------------------------------------------------------

#[derive(Debug, PartialEq, Eq)]
enum OracleError {
    Missing(PathBuf),
    DigestMismatch { expected: String, actual: String },
}

impl std::fmt::Display for OracleError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Missing(path) => write!(
                formatter,
                "the rules CLI is not at {}; run scripts/fetch-rules-oracle",
                path.display()
            ),
            Self::DigestMismatch { expected, actual } => write!(
                formatter,
                "the rules CLI has sha256 {actual}, expected {expected}; re-run scripts/fetch-rules-oracle"
            ),
        }
    }
}

/// The pinned rules CLI, refused unless present with the expected digest.
struct Oracle {
    executable: PathBuf,
}

impl Oracle {
    fn at(executable: &Path, expected_sha256: &str) -> Result<Self, OracleError> {
        let missing = || OracleError::Missing(executable.to_path_buf());
        if !executable.is_file() {
            return Err(missing());
        }
        // `sha256sum` rather than a hashing crate: the check is test-only.
        let output = Command::new("sha256sum")
            .arg(executable)
            .output()
            .map_err(|_| missing())?;
        let actual = String::from_utf8_lossy(&output.stdout)
            .split_whitespace()
            .next()
            .map(str::to_owned)
            .ok_or_else(missing)?;
        if actual != expected_sha256 {
            return Err(OracleError::DigestMismatch {
                expected: expected_sha256.to_owned(),
                actual,
            });
        }
        Ok(Self {
            executable: executable.to_path_buf(),
        })
    }
}

/// The arguments of `battlesnake play` for a standard 11x11 game at the given
/// seed with the listed (name, url) snakes, writing the transcript to `output`.
fn play_args(seed: u64, snakes: &[(&str, &str)], output: &Path) -> Vec<String> {
    let mut args: Vec<String> = [
        "play", "-W", "11", "-H", "11", "-g", "standard", "-m", "standard", "-t", "500", "-r",
    ]
    .map(str::to_owned)
    .to_vec();
    args.push(seed.to_string());
    args.push("-o".to_owned());
    args.push(output.display().to_string());
    for (name, url) in snakes {
        args.extend([
            "--name".to_owned(),
            (*name).to_owned(),
            "--url".to_owned(),
            (*url).to_owned(),
        ]);
    }
    args
}

/// What a game transcript (the CLI's JSON-lines output) says.
#[derive(Debug, Default, PartialEq, Eq)]
struct Transcript {
    game_id: String,
    /// Names of the snakes alive at each turn, in turn order.
    alive: Vec<Vec<String>>,
    winner: Option<String>,
    is_draw: bool,
    completed: bool,
}

/// The first line describes the game, each following line a turn's board, and
/// the last one the result.
fn parse_transcript(text: &str) -> Result<Transcript, String> {
    let mut lines = text.lines().filter(|line| !line.trim().is_empty());
    let first: Value = serde_json::from_str(lines.next().ok_or("the transcript is empty")?)
        .map_err(|error| format!("line 1 is not JSON: {error}"))?;
    let mut transcript = Transcript {
        game_id: first["id"]
            .as_str()
            .ok_or("line 1 has no game id")?
            .to_owned(),
        ..Transcript::default()
    };
    for (index, line) in lines.enumerate() {
        let value: Value = serde_json::from_str(line)
            .map_err(|error| format!("line {} is not JSON: {error}", index + 2))?;
        if let Some(snakes) = value["board"]["snakes"].as_array() {
            transcript.alive.push(
                snakes
                    .iter()
                    .filter_map(|snake| snake["name"].as_str().map(str::to_owned))
                    .collect(),
            );
        } else if value.get("isDraw").is_some() {
            transcript.completed = true;
            transcript.is_draw = value["isDraw"] == true;
            transcript.winner = value["winnerName"]
                .as_str()
                .filter(|name| !name.is_empty())
                .map(str::to_owned);
        }
    }
    Ok(transcript)
}

// ---- the engine's decision log --------------------------------------------------------------

/// One decision the engine logged.
#[derive(Debug, PartialEq, Eq)]
struct Decision {
    game_id: String,
    turn: u32,
    engine_path: String,
}

fn parse_decisions<'a>(lines: impl Iterator<Item = &'a str>) -> Vec<Decision> {
    lines
        .filter_map(|line| {
            let outer: Value = serde_json::from_str(line).ok()?;
            if outer["target"] != "move_decision" {
                return None;
            }
            let event: Value = serde_json::from_str(outer["fields"]["message"].as_str()?).ok()?;
            Some(Decision {
                game_id: event["game_id"].as_str()?.to_owned(),
                turn: u32::try_from(event["turn"].as_u64()?).ok()?,
                engine_path: event["engine_path"].as_str()?.to_owned(),
            })
        })
        .collect()
}

/// The engine path a request must take when `alive` snakes are on the board.
fn expected_path(alive: usize) -> &'static str {
    match alive {
        0 | 1 => "unsupported_fallback",
        2 => "duel_search",
        _ => "melee_search",
    }
}

/// What is wrong with one game: incomplete transcript, a turn the engine was asked
/// about but did not log (or the reverse), or a decision routed to the wrong engine.
fn game_problems(transcript: &Transcript, decisions: &[Decision]) -> Vec<String> {
    let mut found = Vec::new();
    if !transcript.completed {
        found.push("complete: the transcript has no result line".to_owned());
    }
    let ours: Vec<u32> = transcript
        .alive
        .iter()
        .enumerate()
        .filter(|(_, names)| names.iter().any(|n| n == OUR_NAME))
        .map(|(turn, _)| turn as u32)
        .collect();
    let logged: Vec<u32> = decisions.iter().map(|d| d.turn).collect();
    let missing: Vec<&u32> = ours.iter().filter(|t| !logged.contains(t)).collect();
    if !missing.is_empty() {
        found.push(format!("missing: no decision logged for turns {missing:?}"));
    }
    let extra: Vec<u32> = decisions
        .iter()
        .enumerate()
        .filter(|(i, d)| !ours.contains(&d.turn) || logged[..*i].contains(&d.turn))
        .map(|(_, d)| d.turn)
        .collect();
    if !extra.is_empty() {
        found.push(format!(
            "extra: decisions for turns the engine was not asked about, or twice: {extra:?}"
        ));
    }
    let misrouted: Vec<String> = decisions
        .iter()
        .filter(|d| ours.contains(&d.turn))
        .filter(|d| d.engine_path != expected_path(transcript.alive[d.turn as usize].len()))
        .map(|d| format!("turn {} went to {}", d.turn, d.engine_path))
        .collect();
    if !misrouted.is_empty() {
        found.push(format!("route: {}", misrouted.join(", ")));
    }
    found
}

/// The share of decisions, in percent, made by the duel search.
fn duel_share_percent(decisions: &[&Decision]) -> f64 {
    if decisions.is_empty() {
        return 0.0;
    }
    let duel = decisions
        .iter()
        .filter(|d| d.engine_path == "duel_search")
        .count();
    duel as f64 * 100.0 / decisions.len() as f64
}

/// The CLI logs INFO lines for a clean game; anything else (a warning about a
/// snake's response, an error) is a protocol problem.
fn cli_problems(output: &str) -> Vec<String> {
    output
        .lines()
        .filter(|line| !line.trim().is_empty() && !line.starts_with("INFO"))
        .map(|line| format!("cli: {line}"))
        .collect()
}

// ---- fixtures --------------------------------------------------------------------------------

struct Scratch(PathBuf);

impl Scratch {
    fn new(label: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("tiger-endtoend-{}-{label}", std::process::id()));
        fs::create_dir_all(&path).expect("a scratch directory");
        Self(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn state_line(turn: u32, alive: &[&str]) -> String {
    let snakes: Vec<Value> = alive
        .iter()
        .map(|name| serde_json::json!({ "id": format!("id-{name}"), "name": name }))
        .collect();
    serde_json::json!({ "game": { "id": "game-1" }, "turn": turn, "board": { "snakes": snakes } })
        .to_string()
}

fn transcript_text(alive_per_turn: &[&[&str]], result: Option<&str>) -> String {
    let mut lines =
        vec![r#"{"id":"game-1","ruleset":{"name":"standard","version":"cli"}}"#.to_owned()];
    for (turn, alive) in alive_per_turn.iter().enumerate() {
        lines.push(state_line(turn as u32, alive));
    }
    lines.extend(result.map(str::to_owned));
    lines.join("\n")
}

fn log_line(game: &str, turn: u32, path: &str) -> String {
    let event = serde_json::json!({
        "target": "move_decision", "game_id": game, "turn": turn, "engine_path": path
    })
    .to_string();
    serde_json::json!({ "level": "INFO", "fields": { "message": event }, "target": "move_decision" }).to_string()
}

fn decision(turn: u32, path: &str) -> Decision {
    Decision {
        game_id: "game-1".to_owned(),
        turn,
        engine_path: path.to_owned(),
    }
}

// ---- the oracle -------------------------------------------------------------------------------

#[test]
fn a_missing_executable_is_refused_with_the_command_that_fixes_it() {
    let scratch = Scratch::new("missing");
    let path = scratch.0.join("battlesnake");

    let error = Oracle::at(&path, PINNED_EXECUTABLE_SHA256)
        .err()
        .expect("not there");

    assert_eq!(error, OracleError::Missing(path));
    assert!(
        error.to_string().contains("scripts/fetch-rules-oracle"),
        "{error}"
    );
}

#[test]
fn an_executable_with_the_wrong_digest_is_refused_and_both_digests_shown() {
    let scratch = Scratch::new("wrong");
    let path = scratch.0.join("battlesnake");
    fs::write(&path, b"abc").unwrap();

    let error = Oracle::at(&path, PINNED_EXECUTABLE_SHA256)
        .err()
        .expect("wrong digest");

    let OracleError::DigestMismatch { expected, actual } = &error else {
        panic!("expected a digest mismatch, got {error:?}");
    };
    assert_eq!(expected, PINNED_EXECUTABLE_SHA256);
    assert_eq!(
        actual,
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    assert!(error.to_string().contains(actual.as_str()), "{error}");
}

#[test]
fn an_executable_with_the_expected_digest_is_accepted() {
    let scratch = Scratch::new("right");
    let path = scratch.0.join("battlesnake");
    fs::write(&path, b"abc").unwrap();

    let oracle = Oracle::at(
        &path,
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
    )
    .expect("the digest matches");

    assert_eq!(oracle.executable, path);
}

#[test]
fn the_play_arguments_describe_a_standard_11x11_game_at_500_ms() {
    let args = play_args(
        7,
        &[
            ("tiger", "http://127.0.0.1:1"),
            ("base", "http://127.0.0.1:2"),
        ],
        Path::new("/tmp/out.json"),
    );

    let expected = [
        "play",
        "-W",
        "11",
        "-H",
        "11",
        "-g",
        "standard",
        "-m",
        "standard",
        "-t",
        "500",
        "-r",
        "7",
        "-o",
        "/tmp/out.json",
        "--name",
        "tiger",
        "--url",
        "http://127.0.0.1:1",
        "--name",
        "base",
        "--url",
        "http://127.0.0.1:2",
    ];
    assert_eq!(args, expected);
}

// ---- transcripts and logs ------------------------------------------------------------------------

#[test]
fn a_transcript_gives_the_survivors_of_each_turn_and_the_result() {
    let text = transcript_text(
        &[&["tiger", "base"], &["tiger", "base"], &["tiger"]],
        Some(r#"{"winnerId":"id-tiger","winnerName":"tiger","isDraw":false}"#),
    );

    let transcript = parse_transcript(&text).expect("a well-formed transcript");

    assert_eq!(transcript.game_id, "game-1");
    assert_eq!(
        transcript.alive,
        vec![vec!["tiger", "base"], vec!["tiger", "base"], vec!["tiger"]]
    );
    assert_eq!(transcript.winner.as_deref(), Some("tiger"));
    assert!(transcript.completed && !transcript.is_draw);
}

#[test]
fn a_draw_is_a_completed_game_without_a_winner() {
    let text = transcript_text(
        &[&["tiger", "base"], &[]],
        Some(r#"{"winnerId":"","winnerName":"","isDraw":true}"#),
    );

    let transcript = parse_transcript(&text).expect("well formed");

    assert!(transcript.completed && transcript.is_draw);
    assert_eq!(transcript.winner, None);
}

#[test]
fn a_transcript_without_a_result_line_is_not_a_completed_game() {
    let transcript =
        parse_transcript(&transcript_text(&[&["tiger", "base"]], None)).expect("well formed");

    assert!(!transcript.completed);
}

#[test]
fn a_malformed_transcript_is_an_error() {
    assert!(parse_transcript("not json").is_err());
    assert!(parse_transcript("").is_err());
}

#[test]
fn only_move_decision_lines_are_decisions() {
    let lines = [
        log_line("game-1", 0, "duel_search"),
        r#"{"level":"INFO","fields":{"message":"listening","addr":"127.0.0.1:1"},"target":"tiger_engine"}"#.to_owned(),
        "not json".to_owned(),
        log_line("game-1", 1, "unsupported_fallback"),
    ];

    let decisions = parse_decisions(lines.iter().map(String::as_str));

    assert_eq!(
        decisions,
        vec![
            decision(0, "duel_search"),
            decision(1, "unsupported_fallback")
        ]
    );
}

// ---- routing and the duel share ---------------------------------------------------------------------

#[test]
fn the_engine_path_follows_the_number_of_snakes_alive() {
    assert_eq!(expected_path(3), "melee_search");
    assert_eq!(expected_path(4), "melee_search");
    assert_eq!(expected_path(2), "duel_search");
    assert_eq!(expected_path(1), "unsupported_fallback");
}

fn duel_transcript() -> Transcript {
    parse_transcript(&transcript_text(
        &[&["tiger", "base"], &["tiger", "base"], &["tiger"]],
        Some(r#"{"winnerId":"id-tiger","winnerName":"tiger","isDraw":false}"#),
    ))
    .unwrap()
}

#[test]
fn a_game_whose_decisions_match_the_board_has_no_problems() {
    let decisions = [
        decision(0, "duel_search"),
        decision(1, "duel_search"),
        decision(2, "unsupported_fallback"),
    ];

    assert_eq!(
        game_problems(&duel_transcript(), &decisions),
        Vec::<String>::new()
    );
}

#[test]
fn every_way_a_game_can_go_wrong_is_named() {
    let good = || {
        vec![
            decision(0, "duel_search"),
            decision(1, "duel_search"),
            decision(2, "unsupported_fallback"),
        ]
    };
    let mut incomplete = duel_transcript();
    incomplete.completed = false;
    let mut wrong_route = good();
    wrong_route[1].engine_path = "safety_fallback".to_owned();
    let mut missing = good();
    missing.remove(1);
    let mut extra = good();
    extra.push(decision(7, "duel_search"));

    for (name, transcript, decisions) in [
        ("complete", incomplete, good()),
        ("route", duel_transcript(), wrong_route),
        ("missing", duel_transcript(), missing),
        ("extra", duel_transcript(), extra),
    ] {
        let found = game_problems(&transcript, &decisions);

        assert_eq!(found.len(), 1, "{name}: {found:?}");
        assert!(found[0].contains(name), "{name}: {found:?}");
    }
}

#[test]
fn a_three_snake_game_is_checked_against_the_three_way_routing() {
    let transcript = parse_transcript(&transcript_text(
        &[
            &["tiger", "a", "b"],
            &["tiger", "a", "b"],
            &["tiger", "a"],
            &["tiger"],
        ],
        Some(r#"{"winnerId":"id-tiger","winnerName":"tiger","isDraw":false}"#),
    ))
    .unwrap();
    let decisions = [
        decision(0, "melee_search"),
        decision(1, "melee_search"),
        decision(2, "duel_search"),
        decision(3, "unsupported_fallback"),
    ];

    assert_eq!(game_problems(&transcript, &decisions), Vec::<String>::new());
}

#[test]
fn the_duel_share_counts_duel_searches_over_all_decisions() {
    let make = |duel: usize, other: usize| {
        (0..duel)
            .map(|i| decision(i as u32, "duel_search"))
            .chain((0..other).map(|i| decision(1000 + i as u32, "unsupported_fallback")))
            .collect::<Vec<_>>()
    };
    let share = |duel, other| {
        let all = make(duel, other);
        duel_share_percent(&all.iter().collect::<Vec<_>>())
    };

    assert!((share(47, 1) - 97.916_666).abs() < 1e-3);
    assert!(
        share(19, 1) >= DUEL_SHARE_FLOOR_PERCENT,
        "19 of 20 is exactly the floor"
    );
    assert!(share(18, 2) < DUEL_SHARE_FLOOR_PERCENT);
    assert_eq!(share(0, 0), 0.0, "no decisions is no evidence");
}

#[test]
fn any_cli_line_that_is_not_info_is_a_protocol_problem() {
    let clean = "INFO 19:49:56.046 Ruleset: standard, Seed: 7\nINFO 19:49:57.1 Game completed after 48 turns.\n";
    let dirty =
        "INFO fine\nWARN 19:49:57.2 Snake tiger timed out\nERROR could not parse response\n";

    assert!(cli_problems(clean).is_empty());
    let found = cli_problems(dirty);
    assert_eq!(found.len(), 2);
    assert!(found[0].contains("timed out"));
}

// ---- the games (need the CLI, the release binaries and a few minutes) ----------------------------

use std::sync::Mutex;
use std::time::Duration;

use support::server::{ExternalServer, Server};

const DUEL_SEEDS: std::ops::RangeInclusive<u64> = 1..=6;

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("the engine crate sits in the repository")
        .to_path_buf()
}

/// The pinned rules CLI, or a panic that says what to run.
fn pinned_oracle() -> Oracle {
    Oracle::at(
        &repository_root().join(".rules-oracle/battlesnake"),
        PINNED_EXECUTABLE_SHA256,
    )
    .unwrap_or_else(|error| panic!("{error}"))
}

/// The sibling one-turn engine, built into this repository's target directory by
/// `scripts/run-endtoend`.
fn baseline_binary() -> PathBuf {
    let path = repository_root().join("target/baseline/release/battle-snake-rust");
    assert!(
        path.is_file(),
        "the baseline is not built at {}; run scripts/run-endtoend",
        path.display()
    );
    path
}

struct Played {
    transcript: Transcript,
    cli_output: String,
}

/// One game through the rules CLI; panics on a non-zero exit or an unreadable
/// transcript, which are harness failures rather than engine findings.
fn play(oracle: &Oracle, scratch: &Path, seed: u64, snakes: &[(&str, &str)]) -> Played {
    let transcript_path = scratch.join(format!("game-{}-{seed}.jsonl", snakes.len()));
    let output = Command::new(&oracle.executable)
        .args(play_args(seed, snakes, &transcript_path))
        .output()
        .expect("the rules CLI starts");
    assert!(
        output.status.success(),
        "the rules CLI failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = fs::read_to_string(&transcript_path).expect("the transcript was written");
    Played {
        transcript: parse_transcript(&text).expect("a readable transcript"),
        cli_output: format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ),
    }
}

/// Every decision line the engine has logged so far, once it has been quiet for
/// a moment (each event is written before its response is sent).
fn drain_decisions(server: &Server) -> Vec<Decision> {
    let mut lines = Vec::new();
    while let Ok(line) = server.logs.recv_timeout(Duration::from_secs(2)) {
        lines.push(line);
    }
    parse_decisions(lines.iter().map(String::as_str))
}

fn url(addr: std::net::SocketAddr) -> String {
    format!("http://{addr}")
}

#[test]
#[ignore = "plays real games through the rules CLI; run scripts/run-endtoend"]
fn duel_games_against_the_sibling_baseline_use_the_search_almost_always() {
    let oracle = pinned_oracle();
    let baseline = ExternalServer::start(&baseline_binary());
    let ours = Server::start();
    let scratch = Scratch::new("duels");
    let (our_url, baseline_url) = (url(ours.addr), url(baseline.addr));

    let played: Mutex<Vec<(u64, Played)>> = Mutex::new(Vec::new());
    std::thread::scope(|scope| {
        for seed in DUEL_SEEDS {
            let (played, oracle, scratch) = (&played, &oracle, &scratch.0);
            let (our_url, baseline_url) = (&our_url, &baseline_url);
            scope.spawn(move || {
                let game = play(
                    oracle,
                    scratch,
                    seed,
                    &[(OUR_NAME, our_url), ("baseline", baseline_url)],
                );
                played.lock().unwrap().push((seed, game));
            });
        }
    });
    let mut played = played.into_inner().unwrap();
    played.sort_by_key(|(seed, _)| *seed);
    let decisions = drain_decisions(&ours);

    let mut all_problems = Vec::new();
    let mut in_duels: Vec<&Decision> = Vec::new();
    println!("seed | turns | winner | duel decisions | other decisions");
    for (seed, game) in &played {
        let mine: Vec<Decision> = decisions
            .iter()
            .filter(|d| d.game_id == game.transcript.game_id)
            .map(|d| Decision {
                game_id: d.game_id.clone(),
                turn: d.turn,
                engine_path: d.engine_path.clone(),
            })
            .collect();
        let duel = mine
            .iter()
            .filter(|d| d.engine_path == "duel_search")
            .count();
        println!(
            "{seed} | {} | {} | {duel} | {}",
            game.transcript.alive.len(),
            game.transcript.winner.as_deref().unwrap_or("draw"),
            mine.len() - duel
        );
        all_problems.extend(
            game_problems(&game.transcript, &mine)
                .into_iter()
                .map(|p| format!("seed {seed}: {p}")),
        );
        all_problems.extend(
            cli_problems(&game.cli_output)
                .into_iter()
                .map(|p| format!("seed {seed}: {p}")),
        );
        in_duels.extend(
            decisions
                .iter()
                .filter(|d| d.game_id == game.transcript.game_id),
        );
    }
    let share = duel_share_percent(&in_duels);
    println!(
        "decisions: {} | duel share: {share:.2}% (floor {DUEL_SHARE_FLOOR_PERCENT}%)",
        in_duels.len()
    );

    assert_eq!(all_problems, Vec::<String>::new());
    assert!(
        in_duels.len() >= 100,
        "only {} decisions: the games are too short to mean anything",
        in_duels.len()
    );
    assert!(share >= DUEL_SHARE_FLOOR_PERCENT, "duel share {share:.2}%");
}

#[test]
#[ignore = "plays a real game through the rules CLI; run scripts/run-endtoend"]
fn a_three_snake_game_completes_with_the_melee_search() {
    let oracle = pinned_oracle();
    let baseline = ExternalServer::start(&baseline_binary());
    let ours = Server::start();
    let scratch = Scratch::new("melee");
    let baseline_url = url(baseline.addr);

    let game = play(
        &oracle,
        &scratch.0,
        1,
        &[
            (OUR_NAME, &url(ours.addr)),
            ("baseline-a", &baseline_url),
            ("baseline-b", &baseline_url),
        ],
    );
    let decisions: Vec<Decision> = drain_decisions(&ours)
        .into_iter()
        .filter(|d| d.game_id == game.transcript.game_id)
        .collect();

    println!(
        "three-snake game: {} turns, winner {}, {} melee_search / {} duel_search / {} unsupported_fallback decisions",
        game.transcript.alive.len(),
        game.transcript.winner.as_deref().unwrap_or("draw"),
        decisions
            .iter()
            .filter(|d| d.engine_path == "melee_search")
            .count(),
        decisions
            .iter()
            .filter(|d| d.engine_path == "duel_search")
            .count(),
        decisions
            .iter()
            .filter(|d| d.engine_path == "unsupported_fallback")
            .count(),
    );
    assert_eq!(
        game_problems(&game.transcript, &decisions),
        Vec::<String>::new()
    );
    assert_eq!(cli_problems(&game.cli_output), Vec::<String>::new());
    assert!(
        decisions.iter().any(|d| d.engine_path == "melee_search"),
        "no decision was made by the melee search while three snakes were alive"
    );
}
