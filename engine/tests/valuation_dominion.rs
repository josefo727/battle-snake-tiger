mod support;

use std::collections::VecDeque;

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

/// Shortest step counts from `head` through cells that are not occupied.
fn oracle_distances(board: &DuelBoard, head: Cell) -> Vec<Option<u32>> {
    let mut distance = vec![None; usize::from(CELL_COUNT)];
    let mut queue = VecDeque::from([head]);
    distance[usize::from(head.index())] = Some(0);
    while let Some(current) = queue.pop_front() {
        let here = distance[usize::from(current.index())].expect("queued cells have a distance");
        for heading in Heading::ALL {
            let Some(next) = heading.step(current) else {
                continue;
            };
            if board.occupied().contains(next) || distance[usize::from(next.index())].is_some() {
                continue;
            }
            distance[usize::from(next.index())] = Some(here + 1);
            queue.push_back(next);
        }
    }
    distance
}

/// An independent statement of the ownership rule using plain BFS distances.
fn oracle_partition(board: &DuelBoard) -> Partition {
    let ours = oracle_distances(board, board.serpent(Side::Us).head());
    let theirs = oracle_distances(board, board.serpent(Side::Them).head());
    let (our_len, their_len) = (
        board.serpent(Side::Us).length(),
        board.serpent(Side::Them).length(),
    );
    let mut partition = Partition {
        ours: CellSet::EMPTY,
        theirs: CellSet::EMPTY,
    };
    for index in 0..CELL_COUNT {
        let c = Cell::from_index(index).expect("index is on the board");
        if board.occupied().contains(c) {
            continue;
        }
        let (a, b) = (ours[usize::from(index)], theirs[usize::from(index)]);
        let owner = match (a, b) {
            (Some(a), Some(b)) if a < b => Some(Side::Us),
            (Some(a), Some(b)) if b < a => Some(Side::Them),
            (Some(_), Some(_)) => match our_len.cmp(&their_len) {
                std::cmp::Ordering::Greater => Some(Side::Us),
                std::cmp::Ordering::Less => Some(Side::Them),
                std::cmp::Ordering::Equal => None,
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
fn serpent_cells_belong_to_nobody() {
    let b = board(MIRROR_US, MIRROR_THEM);

    let partition = Dominion.partition(&b);

    assert_eq!(partition.ours.intersection(b.occupied()), CellSet::EMPTY);
    assert_eq!(partition.theirs.intersection(b.occupied()), CellSet::EMPTY);
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
fn a_cell_sealed_in_by_body_segments_belongs_to_nobody() {
    // The corner cell (10, 10) has both neighbours, (9, 10) and (10, 9), occupied
    // by body segments (not heads), so neither head can ever enter it.
    let coil: &[(i32, i32)] = &[(7, 10), (8, 10), (9, 10), (9, 9), (10, 9)];
    let them: &[(i32, i32)] = &[(2, 2), (2, 1), (2, 0)];
    let b = board(coil, them);

    let partition = Dominion.partition(&b);

    assert!(!partition.ours.contains(cell(10, 10)));
    assert!(!partition.theirs.contains(cell(10, 10)));
    assert_eq!(partition, oracle_partition(&b));
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
