//! Reading a finished game from the official CLI's JSON-lines transcript.

use serde_json::Value;

/// How a game ended for the challenger.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameOutcome {
    Win,
    Loss,
    Draw,
}

#[derive(Debug, PartialEq, Eq)]
pub struct ParsedGame {
    pub outcome: GameOutcome,
    pub turns: u32,
}

#[derive(Debug, PartialEq, Eq)]
pub enum TranscriptError {
    Empty,
    NotJson { line: usize },
    MissingResult,
    UnknownWinner(String),
}

/// The outcome and length of the game in `text`, from the challenger's side. The
/// first line describes the game, each following board line is a turn and the
/// last line is the result.
///
/// # Errors
///
/// Fails when the transcript is empty, has a line that is not JSON, has no result
/// line, or names a winner that is neither snake.
pub fn parse_transcript(
    text: &str,
    challenger: &str,
    opponent: &str,
) -> Result<ParsedGame, TranscriptError> {
    let mut turns = 0;
    let mut result = None;
    let mut any_line = false;
    for (index, line) in text
        .lines()
        .enumerate()
        .filter(|(_, l)| !l.trim().is_empty())
    {
        any_line = true;
        let value: Value =
            serde_json::from_str(line).map_err(|_| TranscriptError::NotJson { line: index + 1 })?;
        if value.get("isDraw").is_some() {
            result = Some(value);
        } else if value["board"].is_object() {
            turns += 1;
        }
    }
    if !any_line {
        return Err(TranscriptError::Empty);
    }
    let result = result.ok_or(TranscriptError::MissingResult)?;
    let outcome = if result["isDraw"] == true {
        GameOutcome::Draw
    } else {
        match result["winnerName"].as_str().unwrap_or_default() {
            name if name == challenger => GameOutcome::Win,
            name if name == opponent => GameOutcome::Loss,
            name => return Err(TranscriptError::UnknownWinner(name.to_owned())),
        }
    };
    Ok(ParsedGame { outcome, turns })
}
