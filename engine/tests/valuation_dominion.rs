mod support;

use std::cmp::{Ordering, Reverse};
use std::collections::BinaryHeap;

use proptest::prelude::*;
use tiger_engine::arena::cellset::{CELL_COUNT, Cell, CellSet};
use tiger_engine::arena::duel::{DuelBoard, Side};
use tiger_engine::arena::heading::Heading;
use tiger_engine::arena::ingest::ingest;
use tiger_engine::valuation::dominion::{Dominion, Partition};
use tiger_engine::valuation::{Assessor, ValuationPipeline};

use support::{realize, state_spec, turn_state_from_bodies};

fn cell(x: u8, y: u8) -> Cell {
    Cell::from_xy(x, y).expect("test coordinate is on the board")
}

fn board(us: &[(i32, i32)], them: &[(i32, i32)]) -> DuelBoard {
    ingest(&turn_state_from_bodies(&[us, them], &[90, 90], 0, &[])).expect("a duel converts")
}

/// The turn at which each cell first becomes enterable: 0 for a free cell, and
/// for a body cell one more than the highest from-the-tail index of any segment
/// holding it (a stacked tail therefore frees one turn later per copy).
fn release_times(board: &DuelBoard) -> Vec<u32> {
    let mut at = vec![0u32; usize::from(CELL_COUNT)];
    for side in [Side::Us, Side::Them] {
        let serpent = board.serpent(side);
        let length = u32::from(serpent.length());
        for (position, c) in serpent.body().enumerate() {
            let from_tail = length - 1 - u32::try_from(position).expect("small index");
            let slot = &mut at[usize::from(c.index())];
            *slot = (*slot).max(from_tail + 1);
        }
    }
    at
}

/// Earliest arrival at every cell, allowing a serpent to dawdle until a cell
/// frees: `arrival(n) = max(arrival(neighbour) + 1, release(n))`. Independent of
/// the kernel: a plain Dijkstra over a binary heap.
fn oracle_arrivals(source: Cell, release: &[u32]) -> Vec<Option<u32>> {
    let mut best: Vec<Option<u32>> = vec![None; usize::from(CELL_COUNT)];
    let mut heap = BinaryHeap::new();
    best[usize::from(source.index())] = Some(0);
    heap.push(Reverse((0u32, source.index())));
    while let Some(Reverse((arrival, index))) = heap.pop() {
        if best[usize::from(index)] != Some(arrival) {
            continue;
        }
        let here = Cell::from_index(index).expect("index is on the board");
        for heading in Heading::ALL {
            let Some(next) = heading.step(here) else {
                continue;
            };
            let candidate = (arrival + 1).max(release[usize::from(next.index())]);
            let slot = &mut best[usize::from(next.index())];
            if slot.is_none_or(|current| candidate < current) {
                *slot = Some(candidate);
                heap.push(Reverse((candidate, next.index())));
            }
        }
    }
    best
}

/// An independent statement of the ownership rule: earlier arrival wins, equal
/// arrivals go to the longer serpent (nobody at equal length), and the two start
/// cells are never territory.
fn oracle_partition(board: &DuelBoard) -> Partition {
    let release = release_times(board);
    let (us, them) = (board.serpent(Side::Us), board.serpent(Side::Them));
    let ours = oracle_arrivals(us.head(), &release);
    let theirs = oracle_arrivals(them.head(), &release);
    let mut partition = Partition {
        ours: CellSet::EMPTY,
        theirs: CellSet::EMPTY,
    };
    for index in 0..CELL_COUNT {
        let c = Cell::from_index(index).expect("index is on the board");
        if c == us.head() || c == them.head() {
            continue;
        }
        let owner = match (ours[usize::from(index)], theirs[usize::from(index)]) {
            (Some(a), Some(b)) if a < b => Some(Side::Us),
            (Some(a), Some(b)) if b < a => Some(Side::Them),
            (Some(_), Some(_)) => match us.length().cmp(&them.length()) {
                Ordering::Greater => Some(Side::Us),
                Ordering::Less => Some(Side::Them),
                Ordering::Equal => None,
            },
            (Some(_), None) => Some(Side::Us),
            (None, Some(_)) => Some(Side::Them),
            (None, None) => None,
        };
        match owner {
            Some(Side::Us) => partition.ours = partition.ours.with(c),
            Some(Side::Them) => partition.theirs = partition.theirs.with(c),
            None => {}
        }
    }
    partition
}

fn swapped(board: &DuelBoard) -> DuelBoard {
    DuelBoard::try_new(
        *board.serpent(Side::Them),
        *board.serpent(Side::Us),
        board.pellets(),
    )
    .expect("swapping two valid serpents stays valid")
}

const MIRROR_US: &[(i32, i32)] = &[(0, 5), (0, 6), (0, 7)];
const MIRROR_THEM: &[(i32, i32)] = &[(10, 5), (10, 6), (10, 7)];

#[test]
fn the_standard_start_partitions_like_the_independent_oracle() {
    let start = ingest(&support::turn_state(2, 0, &[])).expect("a duel converts");

    assert_eq!(Dominion.partition(&start), oracle_partition(&start));
    assert!(!Dominion.partition(&start).ours.is_empty());
}

#[test]
fn the_start_cells_of_both_heads_are_never_territory() {
    let b = board(MIRROR_US, MIRROR_THEM);

    let partition = Dominion.partition(&b);

    for head in [b.serpent(Side::Us).head(), b.serpent(Side::Them).head()] {
        assert!(!partition.ours.contains(head));
        assert!(!partition.theirs.contains(head));
    }
}

#[test]
fn equal_lengths_leave_the_equidistant_cell_unowned() {
    let b = board(MIRROR_US, MIRROR_THEM);

    let partition = Dominion.partition(&b);

    assert!(!partition.ours.contains(cell(5, 5)));
    assert!(!partition.theirs.contains(cell(5, 5)));
    assert!(partition.ours.contains(cell(4, 5)));
    assert!(partition.theirs.contains(cell(6, 5)));
}

#[test]
fn the_longer_serpent_wins_the_equidistant_cell() {
    let longer_us = board(&[(0, 5), (0, 6), (0, 7), (0, 8)], MIRROR_THEM);
    let longer_them = board(MIRROR_US, &[(10, 5), (10, 6), (10, 7), (10, 8)]);

    assert!(Dominion.partition(&longer_us).ours.contains(cell(5, 5)));
    assert!(Dominion.partition(&longer_them).theirs.contains(cell(5, 5)));
}

#[test]
fn a_serpent_chasing_its_own_tail_owns_the_cells_its_body_vacates() {
    // A 2x2 coil: the tail (4, 5) is beside the head and frees after one turn,
    // then (4, 4) after two and (5, 4) after three, all reachable in that order.
    let coil: &[(i32, i32)] = &[(5, 5), (5, 4), (4, 4), (4, 5)];
    let far: &[(i32, i32)] = &[(9, 9), (9, 10), (10, 10)];
    let b = board(coil, far);

    let partition = Dominion.partition(&b);

    for owned in [cell(4, 5), cell(4, 4), cell(5, 4)] {
        assert!(
            partition.ours.contains(owned),
            "cell ({}, {})",
            owned.x(),
            owned.y()
        );
    }
    assert_eq!(partition, oracle_partition(&b));
}

#[test]
fn a_vacating_tail_beside_the_other_head_goes_to_the_other_serpent() {
    let ours: &[(i32, i32)] = &[(5, 5), (5, 4), (5, 3)];
    let theirs: &[(i32, i32)] = &[(4, 3), (3, 3), (2, 3)];
    let b = board(ours, theirs);

    let partition = Dominion.partition(&b);

    assert!(partition.theirs.contains(cell(5, 3)));
    assert_eq!(partition, oracle_partition(&b));
}

#[test]
fn a_stacked_tail_frees_its_cell_one_turn_later_per_copy() {
    let stacked: &[(i32, i32)] = &[(5, 5), (5, 4), (5, 3), (5, 3), (5, 3)];
    let theirs: &[(i32, i32)] = &[(4, 3), (3, 3), (2, 3)];
    let unstacked: &[(i32, i32)] = &[(5, 5), (5, 4), (5, 3)];

    let with_stack = board(stacked, theirs);
    let without = board(unstacked, theirs);

    assert_eq!(
        Dominion.partition(&with_stack),
        oracle_partition(&with_stack)
    );
    assert_eq!(Dominion.partition(&without), oracle_partition(&without));
    assert_ne!(
        Dominion.partition(&with_stack),
        Dominion.partition(&without),
        "the stack must change who owns what"
    );
}

#[test]
fn the_assessment_is_our_territory_minus_theirs() {
    let b = board(&[(3, 5), (2, 5), (1, 5)], &[(9, 5), (10, 5), (10, 4)]);
    let partition = Dominion.partition(&b);
    let expected = partition.ours.len().cast_signed() - partition.theirs.len().cast_signed();

    assert_eq!(Dominion.assess(&b), expected);
    assert_eq!(
        ValuationPipeline::empty().with(Dominion, 100).score(&b),
        expected * 100
    );
    assert!(expected > 0, "our snake owns the larger side");
}

proptest! {
    #[test]
    fn the_partition_matches_the_independent_oracle_on_generated_duels(spec in state_spec()) {
        let b = ingest(&realize(&spec)).expect("a duel converts");

        prop_assert_eq!(Dominion.partition(&b), oracle_partition(&b));
    }

    #[test]
    fn ownership_is_disjoint_and_swapping_the_serpents_negates_the_assessment(spec in state_spec()) {
        let b = ingest(&realize(&spec)).expect("a duel converts");
        let partition = Dominion.partition(&b);
        let mirrored = Dominion.partition(&swapped(&b));

        prop_assert_eq!(partition.ours.intersection(partition.theirs), CellSet::EMPTY);
        prop_assert_eq!(mirrored.ours, partition.theirs);
        prop_assert_eq!(mirrored.theirs, partition.ours);
        prop_assert_eq!(Dominion.assess(&swapped(&b)), -Dominion.assess(&b));
    }
}
