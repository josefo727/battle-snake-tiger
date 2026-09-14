use std::cell::RefCell;
use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use tiger_sparring::runner::{
    Contestant, Duel, GameResult, OfficialCli, RunnerError, SparringRunner, run_series,
};
use tiger_sparring::transcript::{GameOutcome, TranscriptError};

fn contestant(name: &str, url: &str) -> Contestant {
    Contestant {
        name: name.to_owned(),
        url: url.to_owned(),
    }
}

fn duel(seed: u64) -> Duel {
    Duel {
        challenger: contestant("tiger", "http://127.0.0.1:1"),
        opponent: contestant("base", "http://127.0.0.1:2"),
        seed,
    }
}

/// A scratch directory removed when dropped.
struct Scratch(PathBuf);

impl Scratch {
    fn new(label: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("tiger-sparring-{}-{label}", std::process::id()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

// ---- the series over a fake runner -----------------------------------------------------------

/// Records every call and can be told that a contestant is dead or a seed fails.
struct Fake {
    calls: RefCell<Vec<String>>,
    dead: Option<&'static str>,
    failing_seed: Option<u64>,
}

impl Fake {
    fn healthy() -> Self {
        Self {
            calls: RefCell::new(Vec::new()),
            dead: None,
            failing_seed: None,
        }
    }
}

impl SparringRunner for Fake {
    fn check_alive(&self, contestant: &Contestant) -> Result<(), RunnerError> {
        self.calls
            .borrow_mut()
            .push(format!("alive {}", contestant.name));
        if self.dead == Some(contestant.name.as_str()) {
            return Err(RunnerError::Unreachable {
                name: contestant.name.clone(),
                url: contestant.url.clone(),
                reason: "connection refused".to_owned(),
            });
        }
        Ok(())
    }

    fn play(&self, duel: &Duel) -> Result<GameResult, RunnerError> {
        self.calls.borrow_mut().push(format!("play {}", duel.seed));
        if self.failing_seed == Some(duel.seed) {
            return Err(RunnerError::GameFailed {
                seed: duel.seed,
                reason: "boom".to_owned(),
            });
        }
        Ok(GameResult {
            seed: duel.seed,
            outcome: GameOutcome::Win,
            turns: duel.seed as u32 * 10,
        })
    }
}

fn series(runner: &Fake, seeds: &[u64]) -> Result<Vec<GameResult>, RunnerError> {
    run_series(
        runner,
        &contestant("tiger", "http://a"),
        &contestant("base", "http://b"),
        seeds,
    )
}

#[test]
fn both_contestants_are_proved_alive_before_the_first_game() {
    let runner = Fake::healthy();

    let results = series(&runner, &[1, 2, 3]).expect("a healthy series");

    assert_eq!(
        *runner.calls.borrow(),
        ["alive tiger", "alive base", "play 1", "play 2", "play 3"]
    );
    assert_eq!(
        results.iter().map(|r| r.seed).collect::<Vec<_>>(),
        [1, 2, 3]
    );
    assert_eq!(results[1].turns, 20);
}

#[test]
fn an_unreachable_contestant_fails_the_series_before_any_game() {
    for dead in ["tiger", "base"] {
        let runner = Fake {
            dead: Some(dead),
            ..Fake::healthy()
        };

        let error = series(&runner, &[1, 2]).expect_err("a dead contestant");

        assert!(
            matches!(&error, RunnerError::Unreachable { name, .. } if name == dead),
            "{error:?}"
        );
        assert!(
            runner
                .calls
                .borrow()
                .iter()
                .all(|call| !call.starts_with("play")),
            "no game may start: {:?}",
            runner.calls.borrow()
        );
    }
}

#[test]
fn a_failed_game_stops_the_series_and_names_its_seed() {
    let runner = Fake {
        failing_seed: Some(2),
        ..Fake::healthy()
    };

    let error = series(&runner, &[1, 2, 3]).expect_err("seed 2 fails");

    assert_eq!(
        error,
        RunnerError::GameFailed {
            seed: 2,
            reason: "boom".to_owned()
        }
    );
    assert_eq!(
        *runner.calls.borrow(),
        ["alive tiger", "alive base", "play 1", "play 2"]
    );
}

#[test]
fn a_series_of_no_seeds_is_empty() {
    assert_eq!(series(&Fake::healthy(), &[]), Ok(Vec::new()));
}

// ---- liveness over real HTTP ---------------------------------------------------------------------

/// A one-thread server answering every request with `status`; returns its URL.
fn stub(status: &'static str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    std::thread::spawn(move || {
        for mut stream in listener.incoming().map_while(Result::ok) {
            let mut head = Vec::new();
            let mut buffer = [0u8; 256];
            while !head.windows(4).any(|w| w == b"\r\n\r\n") {
                match stream.read(&mut buffer) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => head.extend_from_slice(&buffer[..n]),
                }
            }
            let body = "{}";
            let _ = write!(
                stream,
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
        }
    });
    url
}

fn cli() -> OfficialCli {
    OfficialCli::new(
        PathBuf::from("/nonexistent/battlesnake"),
        std::env::temp_dir(),
    )
}

#[test]
fn a_server_that_answers_200_is_alive() {
    let url = stub("200 OK");

    assert_eq!(cli().check_alive(&contestant("base", &url)), Ok(()));
}

#[test]
fn a_server_that_answers_anything_else_is_unreachable_and_the_status_is_named() {
    let url = stub("503 Service Unavailable");

    let error = cli()
        .check_alive(&contestant("base", &url))
        .expect_err("a 503");

    let RunnerError::Unreachable {
        name,
        url: reported,
        reason,
    } = error
    else {
        panic!("expected Unreachable");
    };
    assert_eq!((name.as_str(), reported.as_str()), ("base", url.as_str()));
    assert!(reason.contains("503"), "{reason}");
}

#[test]
fn a_closed_port_is_unreachable() {
    let closed = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap();

    let error = cli()
        .check_alive(&contestant("base", &format!("http://{closed}")))
        .expect_err("nothing listens");

    assert!(
        matches!(error, RunnerError::Unreachable { .. }),
        "{error:?}"
    );
}

#[test]
fn a_url_that_is_not_http_is_unreachable_with_a_reason() {
    let error = cli()
        .check_alive(&contestant("base", "ftp://example"))
        .expect_err("not http");

    let RunnerError::Unreachable { reason, .. } = error else {
        panic!("expected Unreachable");
    };
    assert!(reason.contains("http"), "{reason}");
}

/// Executing a script another thread has only just written can fail with "text
/// file busy" if a third thread forks in between, so the tests that write and run
/// a fake CLI take turns.
static FAKE_CLI_TURN: Mutex<()> = Mutex::new(());

fn one_at_a_time() -> MutexGuard<'static, ()> {
    FAKE_CLI_TURN
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

// ---- the official-CLI adapter -----------------------------------------------------------------------

#[test]
fn the_play_arguments_describe_a_standard_11x11_game_at_500_ms() {
    let args = OfficialCli::arguments(&duel(7), Path::new("/tmp/out.jsonl"));

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
        "/tmp/out.jsonl",
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

const WIN_TRANSCRIPT: &str = concat!(
    r#"{"id":"g","ruleset":{"name":"standard","version":"cli"}}"#,
    "\n",
    r#"{"game":{"id":"g"},"turn":0,"board":{"snakes":[{"id":"1","name":"tiger"},{"id":"2","name":"base"}]}}"#,
    "\n",
    r#"{"game":{"id":"g"},"turn":1,"board":{"snakes":[{"id":"1","name":"tiger"}]}}"#,
    "\n",
    r#"{"winnerId":"1","winnerName":"tiger","isDraw":false}"#,
    "\n",
);

/// A stand-in for the CLI: records its arguments and writes `transcript` to the
/// path after `-o`, or exits with `exit_code` without writing anything.
fn fake_cli(dir: &Path, transcript: Option<&str>, exit_code: i32) -> PathBuf {
    let canned = dir.join("canned.jsonl");
    fs::write(&canned, transcript.unwrap_or("")).unwrap();
    let script = dir.join("battlesnake");
    let write_transcript = if transcript.is_some() {
        r#"cat "$CANNED" > "$out""#
    } else {
        ""
    };
    fs::write(
        &script,
        format!(
            "#!/bin/sh\nCANNED='{}'\nout=''\necho \"$@\" > '{}/args.txt'\nwhile [ $# -gt 0 ]; do\n  if [ \"$1\" = '-o' ]; then out=\"$2\"; fi\n  shift\ndone\n{write_transcript}\nexit {exit_code}\n",
            canned.display(),
            dir.display()
        ),
    )
    .unwrap();
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
    script
}

#[test]
fn a_game_is_run_through_the_cli_and_its_transcript_is_read() {
    let _turn = one_at_a_time();
    let scratch = Scratch::new("win");
    let cli = OfficialCli::new(
        fake_cli(&scratch.0, Some(WIN_TRANSCRIPT), 0),
        scratch.0.join("games"),
    );

    let result = cli.play(&duel(9)).expect("the fake CLI plays");

    assert_eq!(
        result,
        GameResult {
            seed: 9,
            outcome: GameOutcome::Win,
            turns: 2
        }
    );
    let args = fs::read_to_string(scratch.0.join("args.txt")).unwrap();
    assert!(
        args.contains("-r 9")
            && args.contains("--name tiger")
            && args.contains("http://127.0.0.1:2"),
        "{args}"
    );
}

#[test]
fn a_cli_that_exits_unsuccessfully_is_a_failed_game() {
    let _turn = one_at_a_time();
    let scratch = Scratch::new("exit");
    let cli = OfficialCli::new(fake_cli(&scratch.0, None, 3), scratch.0.join("games"));

    let error = cli.play(&duel(4)).expect_err("exit code 3");

    let RunnerError::GameFailed { seed, reason } = error else {
        panic!("expected GameFailed");
    };
    assert_eq!(seed, 4);
    assert!(reason.contains('3'), "{reason}");
}

#[test]
fn a_cli_that_writes_no_transcript_is_a_failed_game() {
    let _turn = one_at_a_time();
    let scratch = Scratch::new("silent");
    let cli = OfficialCli::new(fake_cli(&scratch.0, None, 0), scratch.0.join("games"));

    let error = cli.play(&duel(5)).expect_err("no transcript");

    assert!(
        matches!(error, RunnerError::GameFailed { seed: 5, .. }),
        "{error:?}"
    );
}

#[test]
fn an_unreadable_transcript_is_a_typed_error_with_the_seed() {
    let _turn = one_at_a_time();
    let scratch = Scratch::new("garbage");
    let cli = OfficialCli::new(
        fake_cli(&scratch.0, Some("not json\n"), 0),
        scratch.0.join("games"),
    );

    let error = cli.play(&duel(6)).expect_err("garbage");

    assert_eq!(
        error,
        RunnerError::Transcript {
            seed: 6,
            error: TranscriptError::NotJson { line: 1 }
        }
    );
}

#[test]
fn a_missing_executable_is_a_failed_game_that_names_it() {
    let _turn = one_at_a_time();
    let scratch = Scratch::new("missing");
    let cli = OfficialCli::new(scratch.0.join("no-such-cli"), scratch.0.join("games"));

    let error = cli.play(&duel(1)).expect_err("cannot spawn");

    let RunnerError::GameFailed { reason, .. } = error else {
        panic!("expected GameFailed");
    };
    assert!(reason.contains("no-such-cli"), "{reason}");
}
