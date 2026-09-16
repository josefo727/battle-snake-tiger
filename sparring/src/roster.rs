//! Who takes part in a benchmark and how each server is launched.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde_json::Value;

/// How to start one server: a program, its arguments and environment, where the
/// text `{port}` is replaced by the port chosen for this run; and, when killing
/// the program is not enough (a container outlives its `docker run` client),
/// the command that stops the server.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Launch {
    pub program: String,
    pub args: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub stop: Option<Stop>,
}

/// A command run after the server's process is killed, with `{port}` filled in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stop {
    pub program: String,
    pub args: Vec<String>,
}

/// A launch with the port filled in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Command {
    pub program: String,
    pub args: Vec<String>,
    pub env: BTreeMap<String, String>,
    /// The stop command for this port, if the launch has one.
    pub stop: Option<Stop>,
}

impl Launch {
    /// The concrete command for `port`.
    #[must_use]
    pub fn command(&self, port: u16) -> Command {
        let port = port.to_string();
        let fill = |text: &str| text.replace("{port}", &port);
        Command {
            program: fill(&self.program),
            args: self.args.iter().map(|a| fill(a)).collect(),
            env: self.env.iter().map(|(k, v)| (k.clone(), fill(v))).collect(),
            stop: self.stop.as_ref().map(|stop| Stop {
                program: fill(&stop.program),
                args: stop.args.iter().map(|a| fill(a)).collect(),
            }),
        }
    }
}

/// One snake: the name it plays under (also the id) and its launch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub id: String,
    pub launch: Launch,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Roster {
    pub rules_cli_release: String,
    pub challenger: Entry,
    pub baseline: Entry,
    pub opponents: Vec<Entry>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum RosterError {
    NotJson(String),
    Missing(&'static str),
    NoOpponents,
    DuplicateId(String),
    EmptyProgram(String),
    Unreadable(String),
}

impl Roster {
    /// # Errors
    ///
    /// Fails when the text is not a well-formed roster.
    pub fn from_json(text: &str) -> Result<Self, RosterError> {
        let value: Value =
            serde_json::from_str(text).map_err(|error| RosterError::NotJson(error.to_string()))?;
        let rules_cli_release = value["rules_cli_release"]
            .as_str()
            .ok_or(RosterError::Missing("rules_cli_release"))?
            .to_owned();
        let challenger = parse_entry(value.get("challenger"), "challenger")?;
        let baseline = parse_entry(value.get("baseline"), "baseline")?;
        let opponents = value
            .get("opponents")
            .and_then(Value::as_array)
            .ok_or(RosterError::Missing("opponents"))?
            .iter()
            .map(|entry| parse_entry(Some(entry), "opponents"))
            .collect::<Result<Vec<_>, _>>()?;
        if opponents.is_empty() {
            return Err(RosterError::NoOpponents);
        }

        let mut seen = Vec::new();
        for entry in [&challenger, &baseline].into_iter().chain(&opponents) {
            if seen.contains(&entry.id) {
                return Err(RosterError::DuplicateId(entry.id.clone()));
            }
            seen.push(entry.id.clone());
        }
        Ok(Self {
            rules_cli_release,
            challenger,
            baseline,
            opponents,
        })
    }

    /// # Errors
    ///
    /// Fails when the file cannot be read or is not a well-formed roster.
    pub fn load(path: &Path) -> Result<Self, RosterError> {
        let text = fs::read_to_string(path)
            .map_err(|error| RosterError::Unreadable(format!("{}: {error}", path.display())))?;
        Self::from_json(&text)
    }
}

fn parse_entry(value: Option<&Value>, section: &'static str) -> Result<Entry, RosterError> {
    let value = value.ok_or(RosterError::Missing(section))?;
    let id = value["id"]
        .as_str()
        .ok_or(RosterError::Missing(section))?
        .to_owned();
    let program = value["launch"]["program"]
        .as_str()
        .filter(|program| !program.is_empty())
        .ok_or_else(|| RosterError::EmptyProgram(id.clone()))?
        .to_owned();
    let args = value["launch"]["args"]
        .as_array()
        .map(|list| {
            list.iter()
                .filter_map(|a| a.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();
    let env = value["launch"]["env"]
        .as_object()
        .map(|map| {
            map.iter()
                .filter_map(|(k, v)| v.as_str().map(|v| (k.clone(), v.to_owned())))
                .collect()
        })
        .unwrap_or_default();
    let stop = value["launch"]["stop"]
        .as_object()
        .map(|stop| Stop {
            program: stop["program"].as_str().unwrap_or_default().to_owned(),
            args: stop["args"]
                .as_array()
                .map(|list| {
                    list.iter()
                        .filter_map(|a| a.as_str().map(str::to_owned))
                        .collect()
                })
                .unwrap_or_default(),
        })
        .filter(|stop| !stop.program.is_empty());
    Ok(Entry {
        id,
        launch: Launch {
            program,
            args,
            env,
            stop,
        },
    })
}
