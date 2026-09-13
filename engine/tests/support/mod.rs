// Shared across several integration-test binaries; each binary only exercises
// a subset, so an unused helper here is not unused in the suite overall.
#![allow(dead_code)]

pub mod beacon;
pub mod clock;
pub mod schema;

use proptest::prelude::*;
use serde_json::{Value, json};
use tiger_engine::arena::cellset::{Cell, CellSet};
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

/// A request for explicit head-first bodies and health values (Standard v1.2.3,
/// 500 ms timeout, turn 0).
pub fn request_from_bodies(
    bodies: &[&[(i32, i32)]],
    health: &[i32],
    you: usize,
    food: &[(i32, i32)],
) -> TurnRequestDto {
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
    serde_json::from_value(request).expect("the test request must deserialize")
}

/// The reused `TurnState` for explicit head-first bodies and health values.
pub fn turn_state_from_bodies(
    bodies: &[&[(i32, i32)]],
    health: &[i32],
    you: usize,
    food: &[(i32, i32)],
) -> TurnState {
    to_turn_state(&request_from_bodies(bodies, health, you, food))
        .expect("the test state must be valid")
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

/// The reused state that results from a resolved turn: post-move bodies
/// (stacked tails included), post-turn health, and the food still on the board.
pub fn state_after(state: &TurnState, resolution: &TurnResolution) -> TurnState {
    let coordinates = |cell: Cell| (i32::from(cell.x()), i32::from(cell.y()));
    let bodies: Vec<Vec<(i32, i32)>> = resolution
        .snakes()
        .iter()
        .map(|snake| {
            snake
                .body()
                .iter()
                .map(|c| coordinates(Cell::from_index(c.value()).expect("on the board")))
                .collect()
        })
        .collect();
    let health: Vec<i32> = resolution
        .snakes()
        .iter()
        .map(|s| i32::from(s.health()))
        .collect();
    let food: Vec<(i32, i32)> = CellSet::from_bits(resolution.food().bits())
        .iter()
        .map(coordinates)
        .collect();
    let slices: Vec<&[(i32, i32)]> = bodies.iter().map(Vec::as_slice).collect();

    turn_state_from_bodies(&slices, &health, state.you_index(), &food)
}

// --- Generated legal duels, shared by every property suite that needs a position. ---

#[derive(Clone, Debug)]
pub struct SnakeSpec {
    pub start: u8,
    pub turns: Vec<u8>,
    pub tail_stack: u8,
    pub health: u8,
}

#[derive(Clone, Debug)]
pub struct StateSpec {
    pub ours: SnakeSpec,
    pub theirs: SnakeSpec,
    pub food: Vec<u8>,
    pub you_second: bool,
}

pub fn snake_spec() -> impl Strategy<Value = SnakeSpec> {
    (
        0u8..121,
        proptest::collection::vec(0u8..4, 0..24),
        0u8..3,
        1u8..=100,
    )
        .prop_map(|(start, turns, tail_stack, health)| SnakeSpec {
            start,
            turns,
            tail_stack,
            health,
        })
}

pub fn state_spec() -> impl Strategy<Value = StateSpec> {
    (
        snake_spec(),
        snake_spec(),
        proptest::collection::vec(0u8..121, 0..6),
        any::<bool>(),
    )
        .prop_map(|(ours, theirs, food, you_second)| StateSpec {
            ours,
            theirs,
            food,
            you_second,
        })
}

/// Grows a self-avoiding walk from `spec.start` (skipping to the next free cell
/// when the start is taken), turning to the next open heading when blocked, and
/// returns the head-first body with the requested stacked tail copies.
fn realize_snake(spec: &SnakeSpec, taken: &mut CellSet) -> Vec<(i32, i32)> {
    let start = (0..121u8)
        .map(|i| (spec.start + i) % 121)
        .map(|index| Cell::from_index(index).expect("index is on the board"))
        .find(|candidate| !taken.contains(*candidate))
        .expect("a free start cell exists");
    let mut path = vec![start];
    *taken = taken.with(start);
    for &turn in &spec.turns {
        let head = *path.last().expect("path is never empty");
        let next = (0..4usize)
            .map(|offset| Heading::ALL[(usize::from(turn) + offset) % 4])
            .filter_map(|heading| heading.step(head))
            .find(|candidate| !taken.contains(*candidate));
        let Some(next) = next else { break };
        *taken = taken.with(next);
        path.push(next);
    }

    // Standard snakes start at length 3 (coiled on one cell) and never shrink, so
    // shorter bodies are unreachable; the reused resolver even treats a length-1
    // snake eating as a self collision.
    let stack = usize::from(spec.tail_stack).max(3usize.saturating_sub(path.len()));
    let tail = path[0];
    let mut body: Vec<Cell> = path.into_iter().rev().collect();
    body.extend(std::iter::repeat_n(tail, stack));
    body.into_iter()
        .map(|c| (i32::from(c.x()), i32::from(c.y())))
        .collect()
}

pub fn realize(spec: &StateSpec) -> TurnState {
    let mut taken = CellSet::EMPTY;
    let first = realize_snake(&spec.ours, &mut taken);
    let second = realize_snake(&spec.theirs, &mut taken);
    let mut food: Vec<(i32, i32)> = Vec::new();
    for &index in &spec.food {
        let cell = Cell::from_index(index).expect("index is on the board");
        let point = (i32::from(cell.x()), i32::from(cell.y()));
        if !taken.contains(cell) && !food.contains(&point) {
            food.push(point);
        }
    }

    let health = [i32::from(spec.ours.health), i32::from(spec.theirs.health)];
    if spec.you_second {
        turn_state_from_bodies(&[&second, &first], &[health[1], health[0]], 1, &food)
    } else {
        turn_state_from_bodies(&[&first, &second], &health, 0, &food)
    }
}
