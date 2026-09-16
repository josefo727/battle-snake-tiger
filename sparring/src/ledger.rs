//! Sparring reports: assembling a matchup from game results, and persisting a
//! versioned report through a sink.

use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::PathBuf;

use serde_json::{Value, json};

use crate::runner::{BoutResult, GameResult};
use crate::statistics::{StatisticsError, Tally, summarize};
use crate::transcript::GameOutcome;

pub const SCHEMA_VERSION: &str = "1.0.0";
/// The version of `melee-report.schema.json` this module writes.
pub const MELEE_SCHEMA_VERSION: &str = "1.0.0";

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

/// One four-snake game: who sat in seat one, and every snake's placement.
#[derive(Clone, Debug, PartialEq)]
pub struct MeleeGameRecord {
    pub seat_one: String,
    pub seed: u64,
    pub turns: u32,
    /// `(snake id, placement)` in seat order.
    pub placements: Vec<(String, f64)>,
}

impl MeleeGameRecord {
    /// The record of `result`, whose seats were named `names` in order.
    #[must_use]
    pub fn from_bout(names: &[String], result: &BoutResult) -> Self {
        Self {
            seat_one: names[0].clone(),
            seed: result.seed,
            turns: result.turns,
            placements: names
                .iter()
                .cloned()
                .zip(result.placements.iter().copied())
                .collect(),
        }
    }

    fn placement_of(&self, snake: &str) -> Option<f64> {
        self.placements
            .iter()
            .find(|(id, _)| id == snake)
            .map(|(_, place)| *place)
    }
}

/// The mean placement of one opponent beside the challenger and beside the baseline.
#[derive(Clone, Debug, PartialEq)]
pub struct OpponentMeans {
    pub id: String,
    pub mean_with_challenger: f64,
    pub mean_with_baseline: f64,
}

/// The placement benchmark's summary: the means criterion 6 compares.
#[derive(Clone, Debug, PartialEq)]
pub struct MeleeSummary {
    pub challenger: String,
    pub baseline: String,
    /// The roster's first opponent (Sansón).
    pub reference_opponent: String,
    pub challenger_mean: f64,
    pub baseline_mean: f64,
    pub reference_mean_in_challenger_games: f64,
    pub opponents: Vec<OpponentMeans>,
    pub criterion_met: bool,
}

/// The full report of one placement benchmark.
#[derive(Clone, Debug, PartialEq)]
pub struct MeleeReport {
    pub engine: EngineIdentity,
    pub environment: EnvironmentIdentity,
    pub games: Vec<MeleeGameRecord>,
    pub summary: MeleeSummary,
}

impl MeleeReport {
    /// Summarizes `games` (both seatings of every seed) for `challenger`, `baseline`
    /// and `opponents` (the first being the reference), and applies criterion 6:
    /// the challenger's mean placement is strictly below the baseline's and not
    /// above the reference opponent's mean in the challenger's games.
    ///
    /// # Errors
    ///
    /// Fails when the commit is too short, there are no games, or a snake never
    /// played.
    pub fn assemble(
        engine: EngineIdentity,
        environment: EnvironmentIdentity,
        challenger: &str,
        baseline: &str,
        opponents: &[String],
        games: Vec<MeleeGameRecord>,
    ) -> Result<Self, ReportError> {
        if engine.commit.chars().count() < MIN_COMMIT_LENGTH {
            return Err(ReportError::CommitTooShort);
        }
        let mean_of = |snake: &str, seat_one: &str| -> Option<f64> {
            let places: Vec<f64> = games
                .iter()
                .filter(|game| game.seat_one == seat_one)
                .filter_map(|game| game.placement_of(snake))
                .collect();
            (!places.is_empty()).then(|| places.iter().sum::<f64>() / places.len() as f64)
        };
        let challenger_mean = mean_of(challenger, challenger).ok_or(ReportError::NoMatchups)?;
        let baseline_mean = mean_of(baseline, baseline).ok_or(ReportError::NoMatchups)?;
        let reference = opponents.first().ok_or(ReportError::NoMatchups)?;
        let reference_mean_in_challenger_games =
            mean_of(reference, challenger).ok_or(ReportError::NoMatchups)?;
        let means = opponents
            .iter()
            .map(|id| {
                Ok(OpponentMeans {
                    id: id.clone(),
                    mean_with_challenger: mean_of(id, challenger).ok_or(ReportError::NoMatchups)?,
                    mean_with_baseline: mean_of(id, baseline).ok_or(ReportError::NoMatchups)?,
                })
            })
            .collect::<Result<Vec<_>, ReportError>>()?;
        let criterion_met = challenger_mean < baseline_mean
            && challenger_mean <= reference_mean_in_challenger_games;
        Ok(Self {
            engine,
            environment,
            games,
            summary: MeleeSummary {
                challenger: challenger.to_owned(),
                baseline: baseline.to_owned(),
                reference_opponent: reference.clone(),
                challenger_mean,
                baseline_mean,
                reference_mean_in_challenger_games,
                opponents: means,
                criterion_met,
            },
        })
    }

    /// The report as the JSON document of `melee-report.schema.json`.
    #[must_use]
    pub fn to_json(&self) -> Value {
        let games: Vec<Value> = self
            .games
            .iter()
            .map(|game| {
                json!({
                    "seat_one": game.seat_one,
                    "seed": game.seed,
                    "turns": game.turns,
                    "placements": game.placements.iter().map(|(snake, place)| json!({ "snake": snake, "place": place })).collect::<Vec<_>>(),
                })
            })
            .collect();
        let summary = &self.summary;
        json!({
            "schema_version": MELEE_SCHEMA_VERSION,
            "engine": { "name": self.engine.name, "commit": self.engine.commit },
            "environment": {
                "cpu": self.environment.cpu,
                "kernel": self.environment.kernel,
                "rustc": self.environment.rustc,
                "rules_cli_release": self.environment.rules_cli_release,
            },
            "games": games,
            "summary": {
                "challenger": summary.challenger,
                "baseline": summary.baseline,
                "reference_opponent": summary.reference_opponent,
                "challenger_mean": summary.challenger_mean,
                "baseline_mean": summary.baseline_mean,
                "reference_mean_in_challenger_games": summary.reference_mean_in_challenger_games,
                "opponents": summary.opponents.iter().map(|o| json!({
                    "id": o.id,
                    "mean_with_challenger": o.mean_with_challenger,
                    "mean_with_baseline": o.mean_with_baseline,
                })).collect::<Vec<_>>(),
                "criterion_met": summary.criterion_met,
            },
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

/// Where finished placement reports go.
pub trait MeleeReportSink {
    /// # Errors
    ///
    /// Fails when the report cannot be stored.
    fn write_melee(&self, report: &MeleeReport) -> Result<(), SinkError>;
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
        self.write_json(&report.to_json())
    }
}

impl MeleeReportSink for JsonFileSink {
    fn write_melee(&self, report: &MeleeReport) -> Result<(), SinkError> {
        self.write_json(&report.to_json())
    }
}

impl JsonFileSink {
    fn write_json(&self, document: &Value) -> Result<(), SinkError> {
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
        let mut text = serde_json::to_string_pretty(document)
            .map_err(|error| SinkError::Io(error.to_string()))?;
        text.push('\n');
        file.write_all(text.as_bytes())
            .map_err(|error| SinkError::Io(error.to_string()))
    }
}
