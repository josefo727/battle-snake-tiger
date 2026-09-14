//! The command line of the `spar` tool.

use std::path::PathBuf;

#[derive(Debug, PartialEq, Eq)]
pub struct Options {
    pub roster: PathBuf,
    pub oracle: PathBuf,
    pub output: PathBuf,
    pub seeds: Vec<u64>,
    pub workers: usize,
    pub overwrite: bool,
    pub commit: Option<String>,
    pub scratch: PathBuf,
}

#[derive(Debug, PartialEq, Eq)]
pub enum OptionsError {
    MissingOutput,
    UnknownFlag(String),
    MissingValue(String),
    BadValue { flag: String, value: String },
}

impl Options {
    /// # Errors
    ///
    /// Fails on an unknown flag, a flag without its value, a bad value or a
    /// missing `--output`.
    pub fn parse(args: &[String]) -> Result<Self, OptionsError> {
        let mut options = Self {
            roster: PathBuf::from("reference/roster.json"),
            oracle: PathBuf::from(".rules-oracle/battlesnake"),
            output: PathBuf::new(),
            seeds: (1..=30).collect(),
            workers: 1,
            overwrite: false,
            commit: None,
            scratch: PathBuf::from("target/sparring"),
        };
        let mut output = None;
        let mut rest = args.iter();
        while let Some(flag) = rest.next() {
            if flag == "--overwrite" {
                options.overwrite = true;
                continue;
            }
            let known = [
                "--output",
                "--roster",
                "--oracle",
                "--scratch",
                "--seeds",
                "--workers",
                "--commit",
            ];
            if !known.contains(&flag.as_str()) {
                return Err(OptionsError::UnknownFlag(flag.clone()));
            }
            let value = rest
                .next()
                .ok_or_else(|| OptionsError::MissingValue(flag.clone()))?;
            let bad = || OptionsError::BadValue {
                flag: flag.clone(),
                value: value.clone(),
            };
            match flag.as_str() {
                "--output" => output = Some(PathBuf::from(value)),
                "--roster" => options.roster = PathBuf::from(value),
                "--oracle" => options.oracle = PathBuf::from(value),
                "--scratch" => options.scratch = PathBuf::from(value),
                "--commit" => options.commit = Some(value.clone()),
                "--seeds" => options.seeds = parse_seeds(value).ok_or_else(bad)?,
                _ => {
                    options.workers = value.parse().ok().filter(|&w| w >= 1).ok_or_else(bad)?;
                }
            }
        }
        options.output = output.ok_or(OptionsError::MissingOutput)?;
        Ok(options)
    }
}

/// `7` or `1-30`; seeds start at 1 and a range must not run backwards.
fn parse_seeds(text: &str) -> Option<Vec<u64>> {
    let (first, last) = match text.split_once('-') {
        Some((a, b)) => (a.parse::<u64>().ok()?, b.parse::<u64>().ok()?),
        None => {
            let seed = text.parse::<u64>().ok()?;
            (seed, seed)
        }
    };
    (first >= 1 && first <= last).then(|| (first..=last).collect())
}
