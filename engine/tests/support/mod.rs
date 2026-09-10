// Shared across several integration-test binaries; each binary only exercises
// a subset, so an unused helper here is not unused in the suite overall.
#![allow(dead_code)]

use serde_json::{Value, json};
use tiger_engine::arena::cellset::Cell;
use tiger_engine::arena::heading::Heading;
use tiger_engine::rules_core::{
    Direction, JointMoves, TurnRequestDto, TurnResolution, TurnState, resolve_turn, to_turn_state,
};

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

fn snake_from_body(id: usize, body: &[(i32, i32)], health: i32) -> Value {
    let cells: Vec<Value> = body.iter().map(|&(x, y)| coordinate(x, y)).collect();
    json!({
        "id": format!("snake-{id}"),
        "name": format!("snake-{id}"),
        "health": health,
        "body": cells,
        "latency": "0",
        "head": coordinate(body[0].0, body[0].1),
        "length": body.len(),
        "shout": "",
        "customizations": { "color": "#000000", "head": "default", "tail": "default" }
    })
}

/// The reused `TurnState` for explicit head-first bodies and health values.
pub fn turn_state_from_bodies(
    bodies: &[&[(i32, i32)]],
    health: &[i32],
    you: usize,
    food: &[(i32, i32)],
) -> TurnState {
    let snakes: Vec<Value> = bodies
        .iter()
        .enumerate()
        .map(|(i, body)| snake_from_body(i, body, health[i]))
        .collect();
    let food: Vec<Value> = food.iter().map(|&(x, y)| coordinate(x, y)).collect();
    let request = json!({
        "game": {
            "id": "test-game",
            "ruleset": { "name": "standard", "version": "v1.2.3", "settings": {} },
            "map": "standard",
            "timeout": 500,
            "source": "test"
        },
        "turn": 0,
        "board": { "height": 11, "width": 11, "food": food, "hazards": [], "snakes": snakes },
        "you": snake_from_body(you, bodies[you], health[you])
    });
    let request: TurnRequestDto =
        serde_json::from_value(request).expect("the test request must deserialize");
    to_turn_state(&request).expect("the test state must be valid")
}

pub fn to_direction(heading: Heading) -> Direction {
    match heading {
        Heading::North => Direction::Up,
        Heading::East => Direction::Right,
        Heading::South => Direction::Down,
        Heading::West => Direction::Left,
    }
}

/// The reused resolver's answer for one joint move in a two-snake state.
pub fn reference_after(state: &TurnState, us: Heading, them: Heading) -> TurnResolution {
    let you = state.you_index();
    let mut directions = vec![Direction::Up; 2];
    directions[you] = to_direction(us);
    directions[1 - you] = to_direction(them);
    let moves = JointMoves::try_new(directions, 2).expect("two moves for two snakes");
    resolve_turn(state, &moves).expect("the reference resolves the turn")
}

/// A resolved snake's head-first body as kernel cells.
pub fn resolved_body(resolution: &TurnResolution, snake_id: &str) -> Vec<Cell> {
    resolution
        .snake(snake_id)
        .expect("the snake is in the resolution")
        .body()
        .iter()
        .map(|cell| Cell::from_index(cell.value()).expect("reference cells are on the board"))
        .collect()
}
