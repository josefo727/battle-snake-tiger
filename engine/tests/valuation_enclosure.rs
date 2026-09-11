mod support;

use std::cmp::Reverse;
use std::collections::{BinaryHeap, VecDeque};

use proptest::prelude::*;
use tiger_engine::arena::cellset::{CELL_COUNT, Cell, CellSet};
use tiger_engine::arena::duel::{DuelBoard, Side};
use tiger_engine::arena::heading::Heading;
use tiger_engine::arena::ingest::ingest;
use tiger_engine::valuation::enclosure::{Enclosure, parity_bound};
use tiger_engine::valuation::{Assessor, ValuationPipeline};

use support::{realize, state_spec, turn_state_from_bodies};

fn cell(x: u8, y: u8) -> Cell {
    Cell::from_xy(x, y).expect("test coordinate is on the board")
}

fn set_of(coordinates: &[(u8, u8)]) -> CellSet {
    coordinates
        .iter()
        .fold(CellSet::EMPTY, |set, &(x, y)| set.with(cell(x, y)))
}

fn board(us: &[(i32, i32)], them: &[(i32, i32)], vigor: [i32; 2]) -> DuelBoard {
    ingest(&turn_state_from_bodies(&[us, them], &vigor, 0, &[])).expect("a duel converts")
}

fn swapped(board: &DuelBoard) -> DuelBoard {
    DuelBoard::try_new(
        *board.serpent(Side::Them),
        *board.serpent(Side::Us),
        board.pellets(),
    )
    .expect("swapping two valid serpents stays valid")
}

/// Our body is a staircase wall that closes the corner pocket {(0,0), (1,0),
/// (0,1)}; their whole snake sits inside that pocket. Their head (0,1) has no
/// free neighbour; only their own tail can open room for them.
const WALL: &[(i32, i32)] = &[(0, 2), (1, 2), (1, 1), (2, 1), (2, 0)];
const BOXED: &[(i32, i32)] = &[(0, 1), (0, 0), (1, 0)];

/// Cells reachable from `head` through unoccupied cells, by plain BFS.
fn static_region(board: &DuelBoard, head: Cell) -> CellSet {
    let mut seen = CellSet::EMPTY;
    let mut queue = VecDeque::from([head]);
    while let Some(current) = queue.pop_front() {
        for heading in Heading::ALL {
            let Some(next) = heading.step(current) else {
                continue;
            };
            if board.occupied().contains(next) || seen.contains(next) {
                continue;
            }
            seen = seen.with(next);
            queue.push_back(next);
        }
    }
    seen
}

/// The room `side` can reach alone, by an independent Dijkstra: free cells are
/// enterable at once, the serpent's own body cells once their last segment has
/// left, and the other serpent's body never.
fn oracle_room(board: &DuelBoard, side: Side) -> CellSet {
    let serpent = board.serpent(side);
    let mut release = vec![0u32; usize::from(CELL_COUNT)];
    for cell in board.serpent(side.other()).body() {
        release[usize::from(cell.index())] = u32::MAX;
    }
    let length = u32::from(serpent.length());
    for (position, cell) in serpent.body().enumerate() {
        let from_tail = length - 1 - u32::try_from(position).expect("small index");
        let slot = &mut release[usize::from(cell.index())];
        *slot = (*slot).max(from_tail + 1);
    }

    let mut best: Vec<Option<u32>> = vec![None; usize::from(CELL_COUNT)];
    let mut heap = BinaryHeap::new();
    best[usize::from(serpent.head().index())] = Some(0);
    heap.push(Reverse((0u32, serpent.head().index())));
    while let Some(Reverse((arrival, index))) = heap.pop() {
        if best[usize::from(index)] != Some(arrival) {
            continue;
        }
        let here = Cell::from_index(index).expect("index is on the board");
        for heading in Heading::ALL {
            let Some(next) = heading.step(here) else {
                continue;
            };
            let gate = release[usize::from(next.index())];
            if gate == u32::MAX {
                continue;
            }
            let candidate = (arrival + 1).max(gate);
            let slot = &mut best[usize::from(next.index())];
            if slot.is_none_or(|current| candidate < current) {
                *slot = Some(candidate);
                heap.push(Reverse((candidate, next.index())));
            }
        }
    }

    (0..CELL_COUNT)
        .map(|i| Cell::from_index(i).expect("index is on the board"))
        .filter(|c| best[usize::from(c.index())].is_some() && *c != serpent.head())
        .fold(CellSet::EMPTY, CellSet::with)
}

#[test]
fn the_parity_bound_uses_alternating_colours_not_the_raw_cell_count() {
    let head = cell(5, 5);
    // Head colour 0. A line of three cells alternates 1,0,1: all three usable.
    assert_eq!(parity_bound(set_of(&[(6, 5), (7, 5), (8, 5)]), head), 3);
    // A cell of the other colour with two same-colour dead ends: only two usable.
    assert_eq!(parity_bound(set_of(&[(6, 5), (7, 5), (6, 6)]), head), 2);
    // Only same-colour cells: the first step has nowhere to go.
    assert_eq!(parity_bound(set_of(&[(7, 5), (6, 6)]), head), 0);
    assert_eq!(parity_bound(CellSet::EMPTY, head), 0);
}

#[test]
fn a_serpent_boxed_in_with_only_its_own_tail_can_survive_two_turns() {
    let b = board(WALL, BOXED, [90, 90]);

    // Their tail (1, 0) frees on turn 1, then (0, 0) on turn 2: two cells to use.
    assert_eq!(Enclosure.survival_estimate(&b, Side::Them), 2);
}

#[test]
fn hunger_caps_the_estimate() {
    let b = board(WALL, BOXED, [90, 1]);

    assert_eq!(Enclosure.survival_estimate(&b, Side::Them), 1);
}

#[test]
fn walled_apart_serpents_are_compared_by_the_room_each_has() {
    let b = board(WALL, BOXED, [90, 90]);

    let assessment = Enclosure.assess(&b);

    assert_eq!(
        assessment,
        Enclosure.survival_estimate(&b, Side::Us) - Enclosure.survival_estimate(&b, Side::Them)
    );
    assert!(
        assessment > 50,
        "we own the open board and they own two cells: {assessment}"
    );
    assert_eq!(Enclosure.assess(&swapped(&b)), -assessment);
}

#[test]
fn serpents_that_can_reach_each_other_get_no_enclosure_term() {
    let start = ingest(&support::turn_state(2, 0, &[])).expect("a duel converts");

    assert_eq!(Enclosure.assess(&start), 0);
}

#[test]
fn the_term_weights_through_the_pipeline() {
    let b = board(WALL, BOXED, [90, 90]);

    assert_eq!(
        ValuationPipeline::empty().with(Enclosure, 120).score(&b),
        Enclosure.assess(&b) * 120
    );
}

proptest! {
    #[test]
    fn swapping_the_serpents_negates_the_term(spec in state_spec()) {
        let b = ingest(&realize(&spec)).expect("a duel converts");

        prop_assert_eq!(Enclosure.assess(&swapped(&b)), -Enclosure.assess(&b));
    }

    #[test]
    fn a_nonzero_term_implies_the_static_regions_do_not_touch(spec in state_spec()) {
        let b = ingest(&realize(&spec)).expect("a duel converts");
        let ours = static_region(&b, b.serpent(Side::Us).head());
        let theirs = static_region(&b, b.serpent(Side::Them).head());

        if Enclosure.assess(&b) != 0 {
            prop_assert_eq!(ours.intersection(theirs), CellSet::EMPTY);
        }
    }

    #[test]
    fn the_survival_estimate_is_the_parity_bound_of_the_independent_room_capped_by_hunger(
        spec in state_spec(),
    ) {
        let b = ingest(&realize(&spec)).expect("a duel converts");

        for side in [Side::Us, Side::Them] {
            let serpent = b.serpent(side);
            let room = oracle_room(&b, side);
            let pellets = i32::try_from(room.intersection(b.pellets()).len()).expect("small");
            let cap = i32::from(serpent.vigor()) + 100 * pellets;

            prop_assert_eq!(
                Enclosure.survival_estimate(&b, side),
                parity_bound(room, serpent.head()).min(cap)
            );
        }
    }

    #[test]
    fn the_parity_bound_never_exceeds_the_region_size(bits in any::<u128>(), head in 0..CELL_COUNT) {
        let region = CellSet::from_bits(bits);
        let head = Cell::from_index(head).expect("index is on the board");

        prop_assert!(parity_bound(region.without(head), head) <= i32::try_from(region.without(head).len()).expect("small"));
    }
}
