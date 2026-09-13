//! The fixed suite of generated midgame duels shared by the profiling and latency
//! harnesses: reproducible from a seed, played by two greedy-or-wandering snakes.

use tiger_engine::arena::cellset::{Cell, CellSet};
use tiger_engine::arena::duel::{Advance, DuelBoard, Side};
use tiger_engine::arena::heading::Heading;
use tiger_engine::arena::serpent::Serpent;
use tiger_engine::rules_core::TurnState;

use super::{request_value_from_bodies, turn_state_from_bodies};

pub const SUITE_SIZE: usize = 200;
pub const SUITE_SEED: u64 = 0x5EED_2026_0918;

/// splitmix64: the same generator the plan names for Zobrist keys, here for a
/// reproducible suite.
struct SplitMix(u64);

impl SplitMix {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn below(&mut self, bound: u64) -> u64 {
        self.next() % bound
    }

    fn chance(&mut self, percent: u64) -> bool {
        self.below(100) < percent
    }
}

const START_CELLS: [(u8, u8); 8] = [
    (1, 1),
    (1, 5),
    (1, 9),
    (5, 1),
    (5, 9),
    (9, 1),
    (9, 5),
    (9, 9),
];
const OPENING_PELLETS: usize = 14;
/// How often a snake steers toward the nearest pellet instead of wandering.
const GREEDY_PERCENT: u64 = 60;

fn cell(x: u8, y: u8) -> Cell {
    Cell::from_xy(x, y).expect("on the board")
}

/// Two coiled snakes on distinct start cells with pellets scattered around.
fn opening(rng: &mut SplitMix) -> DuelBoard {
    let ours = START_CELLS[rng.below(8) as usize];
    let theirs = loop {
        let candidate = START_CELLS[rng.below(8) as usize];
        if candidate != ours {
            break candidate;
        }
    };
    let coiled = |(x, y): (u8, u8)| {
        let c = cell(x, y);
        Serpent::new(&[c, c, c], 100).expect("a coiled start")
    };
    let mut pellets = CellSet::EMPTY;
    while (pellets.len() as usize) < OPENING_PELLETS {
        let c = cell(rng.below(11) as u8, rng.below(11) as u8);
        if c != cell(ours.0, ours.1) && c != cell(theirs.0, theirs.1) {
            pellets = pellets.with(c);
        }
    }
    DuelBoard::try_new(coiled(ours), coiled(theirs), pellets).expect("a legal opening")
}

/// The four headings for `side`, the one nearest the closest pellet first with
/// probability `GREEDY_PERCENT`, the rest in random order.
fn heading_order(rng: &mut SplitMix, board: &DuelBoard, side: Side) -> [Heading; 4] {
    let mut order = Heading::ALL;
    for i in (1..4).rev() {
        order.swap(i, rng.below(i as u64 + 1) as usize);
    }
    let head = board.serpent(side).head();
    let nearest = board
        .pellets()
        .iter()
        .min_by_key(|p| u32::from(p.x().abs_diff(head.x()) + p.y().abs_diff(head.y())));
    if let Some(target) = nearest
        && rng.chance(GREEDY_PERCENT)
    {
        let gap = |h: &Heading| {
            h.step(head).map_or(u32::MAX, |c| {
                u32::from(c.x().abs_diff(target.x()) + c.y().abs_diff(target.y()))
            })
        };
        order.sort_by_key(gap);
    }
    order
}

/// One ply of play in which both snakes survive, or `None` when no such joint
/// move exists.
fn step(rng: &mut SplitMix, board: &DuelBoard) -> Option<DuelBoard> {
    let ours = heading_order(rng, board, Side::Us);
    let theirs = heading_order(rng, board, Side::Them);
    ours.iter().find_map(|&us| {
        theirs
            .iter()
            .find_map(|&them| match board.advance(us, them) {
                Advance::Continues(next) => Some(next),
                Advance::Over(_) => None,
            })
    })
}

fn play_to_midgame(rng: &mut SplitMix) -> Option<DuelBoard> {
    let target = 20 + rng.below(26);
    let mut board = opening(rng);
    for _ in 0..target {
        board = step(rng, &board)?;
    }
    Some(board)
}

/// 200 midgame duels, reproducible for a seed, played by two greedy-or-wandering
/// snakes that never walk into certain death.
pub fn generate_suite(seed: u64) -> Vec<DuelBoard> {
    let mut rng = SplitMix(seed);
    let mut suite = Vec::with_capacity(SUITE_SIZE);
    while suite.len() < SUITE_SIZE {
        if let Some(board) = play_to_midgame(&mut rng) {
            suite.push(board);
        }
    }
    suite
}

/// A kernel position as the plain data both the reference model and a request
/// body are built from.
struct Snapshot {
    us: Vec<(i32, i32)>,
    them: Vec<(i32, i32)>,
    health: [i32; 2],
    food: Vec<(i32, i32)>,
}

fn snapshot(board: &DuelBoard) -> Snapshot {
    let coordinates = |c: Cell| (i32::from(c.x()), i32::from(c.y()));
    let body = |side| -> Vec<(i32, i32)> { board.serpent(side).body().map(coordinates).collect() };
    Snapshot {
        us: body(Side::Us),
        them: body(Side::Them),
        health: [
            i32::from(board.serpent(Side::Us).vigor()),
            i32::from(board.serpent(Side::Them).vigor()),
        ],
        food: board.pellets().iter().map(coordinates).collect(),
    }
}

/// The reference model's state for a kernel position, our snake first.
pub fn board_to_state(board: &DuelBoard) -> TurnState {
    let p = snapshot(board);
    turn_state_from_bodies(&[&p.us, &p.them], &p.health, 0, &p.food)
}

/// The `/move` request body a server would receive for this position, our
/// snake first and 500 ms declared.
pub fn board_to_request_json(board: &DuelBoard) -> String {
    let p = snapshot(board);
    request_value_from_bodies(&[&p.us, &p.them], &p.health, 0, &p.food).to_string()
}
