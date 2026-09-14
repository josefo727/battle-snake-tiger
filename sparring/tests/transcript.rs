use tiger_sparring::transcript::{GameOutcome, ParsedGame, TranscriptError, parse_transcript};

const RESULT_WIN: &str = r#"{"winnerId":"id-tiger","winnerName":"tiger","isDraw":false}"#;
const RESULT_LOSS: &str = r#"{"winnerId":"id-base","winnerName":"base","isDraw":false}"#;
const RESULT_DRAW: &str = r#"{"winnerId":"","winnerName":"","isDraw":true}"#;

/// A transcript of `turns` turn lines between "tiger" and "base", then `result`.
fn transcript(turns: u32, result: Option<&str>) -> String {
    let mut lines = vec![
        r#"{"id":"game-1","ruleset":{"name":"standard","version":"cli"},"timeout":500}"#.to_owned(),
    ];
    for turn in 0..turns {
        lines.push(format!(
            r#"{{"game":{{"id":"game-1"}},"turn":{turn},"board":{{"snakes":[{{"id":"id-tiger","name":"tiger"}},{{"id":"id-base","name":"base"}}]}}}}"#
        ));
    }
    lines.extend(result.map(str::to_owned));
    lines.join("\n")
}

fn parse(text: &str) -> Result<ParsedGame, TranscriptError> {
    parse_transcript(text, "tiger", "base")
}

#[test]
fn a_win_for_the_challenger_is_a_win_with_its_length() {
    let game = parse(&transcript(48, Some(RESULT_WIN))).expect("a well-formed win");

    assert_eq!(
        game,
        ParsedGame {
            outcome: GameOutcome::Win,
            turns: 48
        }
    );
}

#[test]
fn a_win_for_the_opponent_is_a_loss() {
    let game = parse(&transcript(22, Some(RESULT_LOSS))).expect("a well-formed loss");

    assert_eq!(
        game,
        ParsedGame {
            outcome: GameOutcome::Loss,
            turns: 22
        }
    );
}

#[test]
fn a_draw_is_neither() {
    let game = parse(&transcript(300, Some(RESULT_DRAW))).expect("a well-formed draw");

    assert_eq!(
        game,
        ParsedGame {
            outcome: GameOutcome::Draw,
            turns: 300
        }
    );
}

#[test]
fn the_result_is_read_from_the_challengers_side_whichever_name_wins() {
    let game = parse_transcript(&transcript(10, Some(RESULT_LOSS)), "base", "tiger").unwrap();

    assert_eq!(game.outcome, GameOutcome::Win);
}

#[test]
fn a_transcript_without_a_result_line_is_an_error_not_a_draw() {
    assert_eq!(
        parse(&transcript(5, None)),
        Err(TranscriptError::MissingResult)
    );
}

#[test]
fn an_empty_transcript_is_an_error() {
    assert_eq!(parse(""), Err(TranscriptError::Empty));
    assert_eq!(parse("\n  \n"), Err(TranscriptError::Empty));
}

#[test]
fn a_line_that_is_not_json_is_an_error_naming_its_line() {
    let mut text = transcript(3, Some(RESULT_WIN));
    text = text.replacen("\n", "\nthis is not json\n", 1);

    assert_eq!(parse(&text), Err(TranscriptError::NotJson { line: 2 }));
    assert_eq!(parse("garbage"), Err(TranscriptError::NotJson { line: 1 }));
}

#[test]
fn a_winner_that_is_neither_snake_is_an_error() {
    let stranger = r#"{"winnerId":"id-x","winnerName":"someone-else","isDraw":false}"#;

    assert_eq!(
        parse(&transcript(4, Some(stranger))),
        Err(TranscriptError::UnknownWinner("someone-else".to_owned()))
    );
}
