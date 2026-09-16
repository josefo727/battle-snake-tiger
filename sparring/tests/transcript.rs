use tiger_sparring::transcript::{
    GameOutcome, MeleeGame, ParsedGame, TranscriptError, parse_melee, parse_transcript,
};

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

// ---- several snakes -----------------------------------------------------------------

/// A transcript whose turn `t` lists the snakes of `alive[t]`, then `result`.
fn melee_transcript(alive: &[&[&str]], result: Option<&str>) -> String {
    let mut lines = vec![
        r#"{"id":"game-4","ruleset":{"name":"standard","version":"cli"},"timeout":500}"#.to_owned(),
    ];
    for (turn, names) in alive.iter().enumerate() {
        let snakes: Vec<String> = names
            .iter()
            .map(|n| format!(r#"{{"id":"id-{n}","name":"{n}"}}"#))
            .collect();
        lines.push(format!(
            r#"{{"game":{{"id":"game-4"}},"turn":{turn},"board":{{"snakes":[{}]}}}}"#,
            snakes.join(",")
        ));
    }
    lines.extend(result.map(str::to_owned));
    lines.join("\n")
}

const FOUR: [&str; 4] = ["a", "b", "c", "d"];

#[test]
fn placements_follow_the_elimination_order_and_share_a_turn() {
    // b and d fall on turn 10, c on turn 20, a wins.
    let mut alive: Vec<&[&str]> = Vec::new();
    alive.extend(std::iter::repeat_n(&["a", "b", "c", "d"][..], 10));
    alive.extend(std::iter::repeat_n(&["a", "c"][..], 10));
    alive.push(&["a"][..]);
    let result = r#"{"winnerId":"id-a","winnerName":"a","isDraw":false}"#;

    let game = parse_melee(&melee_transcript(&alive, Some(result)), &FOUR).expect("well-formed");

    assert_eq!(
        game,
        MeleeGame {
            placements: vec![1.0, 3.5, 2.0, 3.5],
            turns: 21
        }
    );
}

#[test]
fn placements_are_reported_in_the_order_the_names_were_given() {
    let alive: Vec<&[&str]> = vec![&["a", "b", "c", "d"], &["b", "c", "d"], &["c"]];
    let result = r#"{"winnerId":"id-c","winnerName":"c","isDraw":false}"#;

    let game = parse_melee(
        &melee_transcript(&alive, Some(result)),
        &["d", "c", "b", "a"],
    )
    .unwrap();

    assert_eq!(game.placements, vec![2.5, 1.0, 2.5, 4.0]);
    assert_eq!(game.turns, 3);
}

#[test]
fn a_draw_at_the_end_shares_first_place_among_the_last_survivors() {
    let alive: Vec<&[&str]> = vec![&["a", "b", "c", "d"], &["a", "b", "c"], &["a", "b"]];
    let result = r#"{"winnerId":"","winnerName":"","isDraw":true}"#;

    let game = parse_melee(&melee_transcript(&alive, Some(result)), &FOUR).unwrap();

    assert_eq!(game.placements, vec![1.5, 1.5, 3.0, 4.0]);
}

#[test]
fn a_snake_that_is_not_a_named_seat_or_a_seat_never_seen_is_an_error() {
    let alive: Vec<&[&str]> = vec![&["a", "b", "c", "x"], &["a"]];
    let result = r#"{"winnerId":"id-a","winnerName":"a","isDraw":false}"#;
    let text = melee_transcript(&alive, Some(result));

    assert_eq!(
        parse_melee(&text, &FOUR),
        Err(TranscriptError::UnknownSnake("x".to_owned()))
    );
    assert_eq!(
        parse_melee(&text, &["a", "b", "c", "x", "e"]),
        Err(TranscriptError::UnknownSnake("e".to_owned()))
    );
}

#[test]
fn the_melee_parser_shares_the_duel_parsers_errors() {
    assert_eq!(parse_melee("", &FOUR), Err(TranscriptError::Empty));
    assert_eq!(
        parse_melee("garbage", &FOUR),
        Err(TranscriptError::NotJson { line: 1 })
    );
    let alive: Vec<&[&str]> = vec![&["a", "b", "c", "d"]];
    assert_eq!(
        parse_melee(&melee_transcript(&alive, None), &FOUR),
        Err(TranscriptError::MissingResult)
    );
    let stranger = r#"{"winnerId":"id-x","winnerName":"someone-else","isDraw":false}"#;
    assert_eq!(
        parse_melee(&melee_transcript(&alive, Some(stranger)), &FOUR),
        Err(TranscriptError::UnknownWinner("someone-else".to_owned()))
    );
}
