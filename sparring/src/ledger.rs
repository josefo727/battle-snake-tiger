//! Sparring reports: assembling a matchup from game results, and persisting a
//! versioned report through a sink.

use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::PathBuf;

use serde_json::{Value, json};

use crate::runner::GameResult;
use crate::statistics::{StatisticsError, Tally, summarize};
use crate::transcript::GameOutcome;

pub const SCHEMA_VERSION: &str = "1.0.0";

/// The engine under test, by name and source revision.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EngineIdentity {
    pub name: String,
    pub commit: String,
}

/// The machine and tools that produced the numbers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnvironmentIdentity {
    pub cpu: String,
    pub kernel: String,
    pub rustc: String,
    pub rules_cli_release: String,
}

/// One challenger against one opponent over a list of seeds.
#[derive(Clone, Debug, PartialEq)]
pub struct Matchup {
    pub challenger: String,
    pub opponent: String,
    pub seeds: Vec<u64>,
    pub wins: u32,
    pub losses: u32,
    pub draws: u32,
    pub win_rate: f64,
    pub wilson95: (f64, f64),
    pub mean_turns: f64,
}

#[derive(Debug, PartialEq)]
pub enum ReportError {
    /// The commit is too short to identify a revision.
    CommitTooShort,
    /// A report needs at least one matchup.
    NoMatchups,
    Statistics(StatisticsError),
}

/// The full report of one benchmark run.
#[derive(Clone, Debug, PartialEq)]
pub struct Report {
    pub engine: EngineIdentity,
    pub environment: EnvironmentIdentity,
    pub matchups: Vec<Matchup>,
}

/// A commit shorter than this cannot identify a revision.
const MIN_COMMIT_LENGTH: usize = 7;

impl Matchup {
    /// Tallies `results` (in seed order) into a matchup.
    ///
    /// # Errors
    ///
    /// Fails when there are no results.
    pub fn assemble(
        challenger: &str,
        opponent: &str,
        results: &[GameResult],
    ) -> Result<Self, ReportError> {
        let count = |wanted| {
            let matching = results.iter().filter(|r| r.outcome == wanted).count();
            u32::try_from(matching).unwrap_or(u32::MAX)
        };
        let tally = Tally {
            wins: count(GameOutcome::Win),
            losses: count(GameOutcome::Loss),
            draws: count(GameOutcome::Draw),
        };
        let summary = summarize(&tally).map_err(ReportError::Statistics)?;
        let turns: u64 = results.iter().map(|r| u64::from(r.turns)).sum();
        Ok(Self {
            challenger: challenger.to_owned(),
            opponent: opponent.to_owned(),
            seeds: results.iter().map(|r| r.seed).collect(),
            wins: tally.wins,
            losses: tally.losses,
            draws: tally.draws,
            win_rate: summary.win_rate,
            wilson95: summary.wilson95,
            mean_turns: turns as f64 / results.len() as f64,
        })
    }

    fn to_json(&self) -> Value {
        json!({
            "challenger": self.challenger,
            "opponent": self.opponent,
            "seeds": self.seeds,
            "wins": self.wins,
            "losses": self.losses,
            "draws": self.draws,
            "win_rate": self.win_rate,
            "win_rate_wilson95": [self.wilson95.0, self.wilson95.1],
            "mean_turns": self.mean_turns,
        })
    }
}

impl Report {
    /// # Errors
    ///
    /// Fails when the commit has fewer than 7 characters or there are no matchups.
    pub fn new(
        engine: EngineIdentity,
        environment: EnvironmentIdentity,
        matchups: Vec<Matchup>,
    ) -> Result<Self, ReportError> {
        if engine.commit.chars().count() < MIN_COMMIT_LENGTH {
            return Err(ReportError::CommitTooShort);
        }
        if matchups.is_empty() {
            return Err(ReportError::NoMatchups);
        }
        Ok(Self {
            engine,
            environment,
            matchups,
        })
    }

    /// The report as the JSON document of `sparring-report.schema.json`.
    #[must_use]
    pub fn to_json(&self) -> Value {
        json!({
            "schema_version": SCHEMA_VERSION,
            "engine": { "name": self.engine.name, "commit": self.engine.commit },
            "environment": {
                "cpu": self.environment.cpu,
                "kernel": self.environment.kernel,
                "rustc": self.environment.rustc,
                "rules_cli_release": self.environment.rules_cli_release,
            },
            "matchups": self.matchups.iter().map(Matchup::to_json).collect::<Vec<_>>(),
        })
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum SinkError {
    /// A report already exists at the path and overwriting was not requested.
    AlreadyExists(PathBuf),
    Io(String),
}

/// Where finished reports go.
pub trait ReportSink {
    /// # Errors
    ///
    /// Fails when the report cannot be stored.
    fn write(&self, report: &Report) -> Result<(), SinkError>;
}

/// A sink that stores the report as a JSON file.
pub struct JsonFileSink {
    path: PathBuf,
    overwrite: bool,
}

impl JsonFileSink {
    /// A sink for `path`; an existing file is replaced only when `overwrite` is true.
    #[must_use]
    pub const fn new(path: PathBuf, overwrite: bool) -> Self {
        Self { path, overwrite }
    }
}

impl ReportSink for JsonFileSink {
    fn write(&self, report: &Report) -> Result<(), SinkError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|error| SinkError::Io(error.to_string()))?;
        }
        // `create_new` makes "refuse to overwrite" atomic: there is no window between
        // looking for the file and creating it.
        let mut options = OpenOptions::new();
        options.write(true);
        if self.overwrite {
            options.create(true).truncate(true);
        } else {
            options.create_new(true);
        }
        let mut file = options
            .open(&self.path)
            .map_err(|error| match error.kind() {
                ErrorKind::AlreadyExists => SinkError::AlreadyExists(self.path.clone()),
                _ => SinkError::Io(error.to_string()),
            })?;
        let mut text = serde_json::to_string_pretty(&report.to_json())
            .map_err(|error| SinkError::Io(error.to_string()))?;
        text.push('\n');
        file.write_all(text.as_bytes())
            .map_err(|error| SinkError::Io(error.to_string()))
    }
}
