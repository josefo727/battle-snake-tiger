//! Running seeded duels between two local servers.

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use crate::transcript::{GameOutcome, TranscriptError, parse_melee, parse_transcript};

/// A snake the runner can reach over HTTP.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Contestant {
    pub name: String,
    pub url: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Duel {
    pub challenger: Contestant,
    pub opponent: Contestant,
    pub seed: u64,
}

/// A game of several snakes: the seats in the order the CLI is given them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bout {
    pub seats: Vec<Contestant>,
    pub seed: u64,
}

/// Each seat's placement (1 is best, shared places averaged), in seat order.
#[derive(Clone, Debug, PartialEq)]
pub struct BoutResult {
    pub seed: u64,
    pub placements: Vec<f64>,
    pub turns: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameResult {
    pub seed: u64,
    pub outcome: GameOutcome,
    pub turns: u32,
}

#[derive(Debug, PartialEq, Eq)]
pub enum RunnerError {
    /// A contestant did not answer `GET /` with a 200.
    Unreachable {
        name: String,
        url: String,
        reason: String,
    },
    /// The game process could not be run or exited unsuccessfully.
    GameFailed { seed: u64, reason: String },
    /// The game ran but its transcript could not be read.
    Transcript { seed: u64, error: TranscriptError },
}

/// Something that can play a duel and vouch that a contestant is alive.
pub trait SparringRunner {
    /// # Errors
    ///
    /// Fails when the contestant does not answer `GET /` with a 200.
    fn check_alive(&self, contestant: &Contestant) -> Result<(), RunnerError>;

    /// # Errors
    ///
    /// Fails when the game cannot be run or its transcript cannot be read.
    fn play(&self, duel: &Duel) -> Result<GameResult, RunnerError>;
}

/// Something that can play a bout of several snakes (liveness checks come
/// from [`SparringRunner`]).
pub trait MeleeRunner {
    /// # Errors
    ///
    /// Fails when the game cannot be run or its transcript cannot be read.
    fn play_bout(&self, bout: &Bout) -> Result<BoutResult, RunnerError>;
}

/// Plays one duel per seed, in order, after both contestants have proved alive.
///
/// # Errors
///
/// Stops at the first unreachable contestant (before any game) or failed game.
pub fn run_series<R: SparringRunner>(
    runner: &R,
    challenger: &Contestant,
    opponent: &Contestant,
    seeds: &[u64],
) -> Result<Vec<GameResult>, RunnerError> {
    runner.check_alive(challenger)?;
    runner.check_alive(opponent)?;
    seeds
        .iter()
        .map(|&seed| {
            runner.play(&Duel {
                challenger: challenger.clone(),
                opponent: opponent.clone(),
                seed,
            })
        })
        .collect()
}

/// The official `battlesnake play` CLI as a runner.
pub struct OfficialCli {
    executable: PathBuf,
    scratch: PathBuf,
}

impl OfficialCli {
    #[must_use]
    pub const fn new(executable: PathBuf, scratch: PathBuf) -> Self {
        Self {
            executable,
            scratch,
        }
    }

    /// The `battlesnake play` arguments for `bout`, writing the transcript to
    /// `output`: one `--name`/`--url` pair per seat, in seat order.
    #[must_use]
    pub fn bout_arguments(bout: &Bout, output: &Path) -> Vec<String> {
        let mut args: Vec<String> = [
            "play", "-W", "11", "-H", "11", "-g", "standard", "-m", "standard", "-t", "500", "-r",
        ]
        .map(str::to_owned)
        .to_vec();
        args.push(bout.seed.to_string());
        args.push("-o".to_owned());
        args.push(output.display().to_string());
        for snake in &bout.seats {
            args.extend([
                "--name".to_owned(),
                snake.name.clone(),
                "--url".to_owned(),
                snake.url.clone(),
            ]);
        }
        args
    }

    /// The `battlesnake play` arguments for `duel`, writing the transcript to `output`.
    #[must_use]
    pub fn arguments(duel: &Duel, output: &Path) -> Vec<String> {
        Self::bout_arguments(&Self::as_bout(duel), output)
    }

    /// A duel is the two-seat bout, challenger first.
    fn as_bout(duel: &Duel) -> Bout {
        Bout {
            seats: vec![duel.challenger.clone(), duel.opponent.clone()],
            seed: duel.seed,
        }
    }

    /// Runs the CLI for `bout` and returns the transcript it wrote. Everything about
    /// processes and files lives here; reading the transcript does not.
    fn run_cli(&self, bout: &Bout) -> Result<String, RunnerError> {
        let failed = |reason: String| RunnerError::GameFailed {
            seed: bout.seed,
            reason,
        };
        let output = self.transcript_path(bout);
        fs::create_dir_all(&self.scratch).map_err(|error| {
            failed(format!("cannot create {}: {error}", self.scratch.display()))
        })?;
        // A stale transcript from an earlier run must never stand in for this game.
        let _ = fs::remove_file(&output);

        let status = Command::new(&self.executable)
            .args(Self::bout_arguments(bout, &output))
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(|error| {
                failed(format!("cannot run {}: {error}", self.executable.display()))
            })?;
        if !status.success() {
            return Err(failed(format!("the CLI exited with {status}")));
        }
        fs::read_to_string(&output).map_err(|_| {
            failed(format!(
                "the CLI wrote no transcript at {}",
                output.display()
            ))
        })
    }

    /// Where the transcript of `bout` is written: one file per seating and seed.
    fn transcript_path(&self, bout: &Bout) -> PathBuf {
        let safe = |name: &str| -> String {
            name.chars()
                .map(|c| {
                    if c.is_ascii_alphanumeric() || c == '-' {
                        c
                    } else {
                        '_'
                    }
                })
                .collect()
        };
        let names: Vec<String> = bout.seats.iter().map(|s| safe(&s.name)).collect();
        self.scratch
            .join(format!("{}-{}.jsonl", names.join("-vs-"), bout.seed))
    }
}

impl MeleeRunner for OfficialCli {
    fn play_bout(&self, bout: &Bout) -> Result<BoutResult, RunnerError> {
        let text = self.run_cli(bout)?;
        let names: Vec<&str> = bout.seats.iter().map(|s| s.name.as_str()).collect();
        let game = parse_melee(&text, &names).map_err(|error| RunnerError::Transcript {
            seed: bout.seed,
            error,
        })?;
        Ok(BoutResult {
            seed: bout.seed,
            placements: game.placements,
            turns: game.turns,
        })
    }
}

impl SparringRunner for OfficialCli {
    fn check_alive(&self, contestant: &Contestant) -> Result<(), RunnerError> {
        probe(&contestant.url).map_err(|reason| RunnerError::Unreachable {
            name: contestant.name.clone(),
            url: contestant.url.clone(),
            reason,
        })
    }

    fn play(&self, duel: &Duel) -> Result<GameResult, RunnerError> {
        let text = self.run_cli(&Self::as_bout(duel))?;
        let game = parse_transcript(&text, &duel.challenger.name, &duel.opponent.name).map_err(
            |error| RunnerError::Transcript {
                seed: duel.seed,
                error,
            },
        )?;
        Ok(GameResult {
            seed: duel.seed,
            outcome: game.outcome,
            turns: game.turns,
        })
    }
}

/// `GET /` on an `http://host[:port]` URL: alive only if the answer is a 200.
fn probe(url: &str) -> Result<(), String> {
    const PATIENCE: Duration = Duration::from_secs(2);
    let authority = url
        .strip_prefix("http://")
        .ok_or_else(|| format!("not an http URL: {url}"))?
        .trim_end_matches('/');
    let address = if authority.contains(':') {
        authority.to_owned()
    } else {
        format!("{authority}:80")
    };
    let target = address
        .to_socket_addrs()
        .map_err(|error| format!("cannot resolve {address}: {error}"))?
        .next()
        .ok_or_else(|| format!("no address for {address}"))?;
    let mut stream =
        TcpStream::connect_timeout(&target, PATIENCE).map_err(|error| error.to_string())?;
    stream
        .set_read_timeout(Some(PATIENCE))
        .map_err(|error| error.to_string())?;
    let request = format!("GET / HTTP/1.1\r\nHost: {authority}\r\nConnection: close\r\n\r\n");
    stream
        .write_all(request.as_bytes())
        .map_err(|error| error.to_string())?;
    let mut line = String::new();
    BufReader::new(&stream)
        .read_line(&mut line)
        .map_err(|error| error.to_string())?;
    let line = line.trim_end();
    match line.split_whitespace().nth(1) {
        Some("200") => Ok(()),
        _ => Err(format!("GET / answered `{line}`")),
    }
}
