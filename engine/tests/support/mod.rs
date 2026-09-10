// Shared across several integration-test binaries; each binary only exercises
// a subset, so an unused helper here is not unused in the suite overall.
#![allow(dead_code)]

use serde_json::{Value, json};
use tiger_engine::rules_core::{TurnRequestDto, TurnState, to_turn_state};

const BODIES: [[(i32, i32); 3]; 4] = [
    [(5, 5), (5, 4), (5, 3)],
    [(2, 2), (2, 1), (2, 0)],
    [(9, 9), (9, 8), (9, 7)],
    [(1, 9), (1, 8), (1, 7)],
];

fn coordinate(x: i32, y: i32) -> Value {
    json!({ "x": x, "y": y })
}

fn snake_json(index: usize) -> Value {
    let body: Vec<Value> = BODIES[index]
        .iter()
        .map(|&(x, y)| coordinate(x, y))
        .collect();
    let (hx, hy) = BODIES[index][0];
    json!({
        "id": format!("snake-{index}"),
        "name": format!("snake-{index}"),
        "health": 90,
        "body": body,
        "latency": "0",
        "head": coordinate(hx, hy),
        "length": 3,
        "shout": "",
        "customizations": { "color": "#000000", "head": "default", "tail": "default" }
    })
}

/// A Standard, non-wrapped, hazard-free 11x11 request with `snakes` snakes,
/// the given ruleset version, declared timeout, `you` index, and food cells.
pub fn request_json_with(
    version: &str,
    snakes: usize,
    timeout: i64,
    you: usize,
    food: &[(i32, i32)],
) -> String {
    let all: Vec<Value> = (0..snakes).map(snake_json).collect();
    let food: Vec<Value> = food.iter().map(|&(x, y)| coordinate(x, y)).collect();
    json!({
        "game": {
            "id": "test-game",
            "ruleset": { "name": "standard", "version": version, "settings": {} },
            "map": "standard",
            "timeout": timeout,
            "source": "test"
        },
        "turn": 0,
        "board": { "height": 11, "width": 11, "food": food, "hazards": [], "snakes": all },
        "you": snake_json(you)
    })
    .to_string()
}

/// Like [`request_json_with`] with our snake first and no food.
pub fn request_json(version: &str, snakes: usize, timeout: i64) -> String {
    request_json_with(version, snakes, timeout, 0, &[])
}

pub fn request_with(
    version: &str,
    snakes: usize,
    timeout: i64,
    you: usize,
    food: &[(i32, i32)],
) -> TurnRequestDto {
    serde_json::from_str(&request_json_with(version, snakes, timeout, you, food))
        .expect("the test request must deserialize")
}

pub fn request(version: &str, snakes: usize, timeout: i64) -> TurnRequestDto {
    request_with(version, snakes, timeout, 0, &[])
}

/// The reused `TurnState` for a test request (2 to 4 snakes).
pub fn turn_state(snakes: usize, you: usize, food: &[(i32, i32)]) -> TurnState {
    to_turn_state(&request_with("v1.2.3", snakes, 500, you, food))
        .expect("the test state must be valid")
}
