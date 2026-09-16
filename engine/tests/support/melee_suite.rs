//! A fixed suite of generated midgame melees (three or four snakes) shared by
//! the profiling and latency harnesses: reproducible from a seed, played by
//! greedy-or-wandering snakes that never walk into certain death.

use tiger_engine::arena::cellset::{Cell, CellSet};
use tiger_engine::arena::heading::Heading;
use tiger_engine::arena::melee::{MAX_SEATS, MeleeBoard, MeleeOutcome, Seat};
use tiger_engine::arena::serpent::Serpent;
use tiger_engine::rules_core::TurnState;

use super::{request_value_from_bodies, turn_state_from_bodies};

pub const MELEE_SUITE_SIZE: usize = 100;
pub const MELEE_SUITE_SEED: u64 = 0x5EED_2026_0919;

/// splitmix64, as the duel suite uses.
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
const GREEDY_PERCENT: u64 = 60;

fn cell(x: u8, y: u8) -> Cell {
    Cell::from_xy(x, y).expect("on the board")
}

/// `snakes` coiled snakes on distinct start cells with pellets scattered around.
fn opening(rng: &mut SplitMix, snakes: usize) -> MeleeBoard {
    let mut starts: Vec<(u8, u8)> = Vec::new();
    while starts.len() < snakes {
        let candidate = START_CELLS[rng.below(8) as usize];
        if !starts.contains(&candidate) {
            starts.push(candidate);
        }
    }
    let serpents: Vec<Serpent> = starts
        .iter()
        .map(|&(x, y)| {
            let c = cell(x, y);
            Serpent::new(&[c, c, c], 100).expect("a coiled start")
        })
        .collect();
    let taken = starts.iter().map(|&(x, y)| cell(x, y)).collect::<Vec<_>>();
    let mut pellets = CellSet::EMPTY;
    while (pellets.len() as usize) < OPENING_PELLETS {
        let c = cell(rng.below(11) as u8, rng.below(11) as u8);
        if !taken.contains(&c) {
            pellets = pellets.with(c);
        }
    }
    MeleeBoard::try_new(&serpents, pellets).expect("a legal opening")
}

/// The four headings for `seat`, the one nearest the closest pellet first with
/// probability `GREEDY_PERCENT`, the rest in random order.
fn heading_order(rng: &mut SplitMix, board: &MeleeBoard, seat: Seat) -> [Heading; 4] {
    let mut order = Heading::ALL;
    for i in (1..4).rev() {
        order.swap(i, rng.below(i as u64 + 1) as usize);
    }
    let head = board.serpent(seat).head();
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

/// One ply of play in which every snake survives, or `None` when no such joint
/// move exists among the tried combinations (each seat tries its headings in
/// order, the first seats varying slowest).
fn step(rng: &mut SplitMix, board: &MeleeBoard) -> Option<MeleeBoard> {
    let seats: Vec<Seat> = board.seats().collect();
    let orders: Vec<[Heading; 4]> = seats
        .iter()
        .map(|&seat| heading_order(rng, board, seat))
        .collect();
    let total = 4usize.pow(seats.len() as u32);
    (0..total).find_map(|code| {
        let mut moves = [Heading::North; MAX_SEATS];
        for (k, seat) in seats.iter().enumerate() {
            moves[seat.index()] = orders[k][(code >> (2 * (seats.len() - 1 - k))) & 3];
        }
        match board.advance(&moves) {
            MeleeOutcome::Continues(next) if usize::from(next.alive_count()) == seats.len() => {
                Some(next)
            }
            _ => None,
        }
    })
}

fn play_to_midgame(rng: &mut SplitMix, snakes: usize) -> Option<MeleeBoard> {
    let target = 20 + rng.below(26);
    let mut board = opening(rng, snakes);
    for _ in 0..target {
        board = step(rng, &board)?;
    }
    (!board.pellets().is_empty()).then_some(board)
}

/// `size` midgame melees of `snakes` snakes (ply 20 to 45), reproducible for a
/// seed, every snake alive and some food left.
pub fn generate_melee_suite(seed: u64, snakes: usize, size: usize) -> Vec<MeleeBoard> {
    let mut rng = SplitMix(seed);
    let mut suite = Vec::with_capacity(size);
    while suite.len() < size {
        if let Some(board) = play_to_midgame(&mut rng, snakes) {
            suite.push(board);
        }
    }
    suite
}

struct Snapshot {
    bodies: Vec<Vec<(i32, i32)>>,
    health: Vec<i32>,
    food: Vec<(i32, i32)>,
}

fn snapshot(board: &MeleeBoard) -> Snapshot {
    let coordinates = |c: Cell| (i32::from(c.x()), i32::from(c.y()));
    Snapshot {
        bodies: board
            .seats()
            .map(|seat| board.serpent(seat).body().map(coordinates).collect())
            .collect(),
        health: board
            .seats()
            .map(|seat| i32::from(board.serpent(seat).vigor()))
            .collect(),
        food: board.pellets().iter().map(coordinates).collect(),
    }
}

/// The reference model's state for a melee position, our snake first.
pub fn melee_board_to_state(board: &MeleeBoard) -> TurnState {
    let p = snapshot(board);
    let slices: Vec<&[(i32, i32)]> = p.bodies.iter().map(Vec::as_slice).collect();
    turn_state_from_bodies(&slices, &p.health, 0, &p.food)
}

/// The `/move` request body a server would receive for this position, our
/// snake first and 500 ms declared.
pub fn melee_board_to_request_json(board: &MeleeBoard) -> String {
    let p = snapshot(board);
    let slices: Vec<&[(i32, i32)]> = p.bodies.iter().map(Vec::as_slice).collect();
    request_value_from_bodies(&slices, &p.health, 0, &p.food).to_string()
}
