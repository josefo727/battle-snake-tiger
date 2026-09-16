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
    NotJson {
        line: usize,
    },
    MissingResult,
    UnknownWinner(String),
    /// A snake on the board (or expected on it) is not one of the named seats.
    UnknownSnake(String),
}

/// A finished game of several snakes: each named snake's placement (1 is best;
/// snakes eliminated in the same turn share the average of their places) in
/// the order the names were given, and the number of turns played.
#[derive(Debug, PartialEq)]
pub struct MeleeGame {
    pub placements: Vec<f64>,
    pub turns: u32,
}

/// The placements of `names` in the game in `text`: the winner (or the snakes
/// that died together at the end) first, then by the turn each snake was last
/// on the board.
///
/// # Errors
///
/// Fails like [`parse_transcript`], and when a snake on the board is not one of
/// `names` or one of `names` never appears.
pub fn parse_melee(text: &str, names: &[&str]) -> Result<MeleeGame, TranscriptError> {
    let lines = read_lines(text)?;
    let mut last_seen: Vec<Option<u32>> = vec![None; names.len()];
    for (turn, board) in lines.boards.iter().enumerate() {
        for snake in board["board"]["snakes"].as_array().into_iter().flatten() {
            let name = snake["name"].as_str().unwrap_or_default();
            let seat = names
                .iter()
                .position(|n| *n == name)
                .ok_or_else(|| TranscriptError::UnknownSnake(name.to_owned()))?;
            last_seen[seat] = Some(turn as u32);
        }
    }
    if let Some(unseen) = last_seen.iter().position(Option::is_none) {
        return Err(TranscriptError::UnknownSnake(names[unseen].to_owned()));
    }
    if lines.result["isDraw"] != true {
        let winner = lines.result["winnerName"].as_str().unwrap_or_default();
        if !names.contains(&winner) {
            return Err(TranscriptError::UnknownWinner(winner.to_owned()));
        }
    }

    // Later last turns place better; seats sharing a last turn share the
    // average of the places they would have taken.
    let placements = names
        .iter()
        .enumerate()
        .map(|(seat, _)| {
            let mine = last_seen[seat].expect("every seat was seen");
            let better = last_seen.iter().filter(|t| t.unwrap() > mine).count();
            let tied = last_seen.iter().filter(|t| t.unwrap() == mine).count();
            better as f64 + (tied as f64 + 1.0) / 2.0
        })
        .collect();
    Ok(MeleeGame {
        placements,
        turns: lines.boards.len() as u32,
    })
}

/// The parsed lines of a transcript: every board line and the result line.
struct Lines {
    boards: Vec<Value>,
    result: Value,
}

fn read_lines(text: &str) -> Result<Lines, TranscriptError> {
    let mut boards = Vec::new();
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
            boards.push(value);
        }
    }
    if !any_line {
        return Err(TranscriptError::Empty);
    }
    let result = result.ok_or(TranscriptError::MissingResult)?;
    Ok(Lines { boards, result })
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
    let lines = read_lines(text)?;
    let outcome = if lines.result["isDraw"] == true {
        GameOutcome::Draw
    } else {
        match lines.result["winnerName"].as_str().unwrap_or_default() {
            name if name == challenger => GameOutcome::Win,
            name if name == opponent => GameOutcome::Loss,
            name => return Err(TranscriptError::UnknownWinner(name.to_owned())),
        }
    };
    Ok(ParsedGame {
        outcome,
        turns: lines.boards.len() as u32,
    })
}
