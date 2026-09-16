mod support;

use proptest::prelude::*;
use tiger_engine::arena::cellset::{Cell, CellSet};
use tiger_engine::arena::heading::Heading;
use tiger_engine::arena::ingest::{IngestError, ingest_melee};
use tiger_engine::arena::melee::{MAX_SEATS, MeleeBoard, MeleeOutcome, Seat};
use tiger_engine::rules_core::{TurnResolution, TurnState};

use support::{
    headings_in_state_order, melee_spec, realize_melee, reference_after_all, resolved_body,
    seat_ids, turn_state, turn_state_from_bodies,
};

fn cell(x: u8, y: u8) -> Cell {
    Cell::from_xy(x, y).expect("test coordinate is on the board")
}

fn moves(by_seat: &[Heading]) -> [Heading; MAX_SEATS] {
    let mut all = [Heading::North; MAX_SEATS];
    all[..by_seat.len()].copy_from_slice(by_seat);
    all
}

fn continued(board: &MeleeBoard, by_seat: &[Heading]) -> MeleeBoard {
    match board.advance(&moves(by_seat)) {
        MeleeOutcome::Continues(next) => next,
        ended => panic!("the game unexpectedly ended: {ended:?}"),
    }
}

/// Who survives according to the reused resolver, by seat.
fn reference_survivors(state: &TurnState, by_seat: &[Heading]) -> Vec<bool> {
    let reference = reference_after_all(state, &headings_in_state_order(state, by_seat));
    seat_ids(state)
        .iter()
        .map(|id| reference.snake(id).expect("resolved").survived())
        .collect()
}

/// Asserts the kernel ends (or continues) the melee exactly as the reference
/// implies, and that survivors agree with it.
fn assert_outcome_matches(state: &TurnState, by_seat: &[Heading]) {
    let board = ingest_melee(state).expect("a melee converts");
    let survivors = reference_survivors(state, by_seat);
    let we_live = survivors[0];
    let rivals_left = survivors[1..].iter().filter(|alive| **alive).count() as u8;
    let label = format!("{by_seat:?}");

    match board.advance(&moves(by_seat)) {
        MeleeOutcome::Continues(next) => {
            assert!(
                we_live && rivals_left >= 1,
                "{label}: kernel continues, reference has survivors {survivors:?}"
            );
            let alive: Vec<bool> = Seat::ALL[..by_seat.len()]
                .iter()
                .map(|s| next.is_alive(*s))
                .collect();
            assert_eq!(alive, survivors, "{label}: survivors");
            let reference = reference_after_all(state, &headings_in_state_order(state, by_seat));
            assert_board_matches(&next, state, &reference, &label);
        }
        MeleeOutcome::WeAlone => {
            assert!(
                we_live && rivals_left == 0,
                "{label}: kernel says we are alone, reference {survivors:?}"
            );
        }
        MeleeOutcome::WeDown { rivals_left: got } => {
            assert!(
                !we_live,
                "{label}: kernel says we died, reference {survivors:?}"
            );
            assert_eq!(got, rivals_left, "{label}: rivals left");
        }
    }
}

/// Asserts that every living seat of `board` matches the reused resolver's
/// body and health for the same snake, and that the pellets agree.
fn assert_board_matches(
    board: &MeleeBoard,
    state: &TurnState,
    reference: &TurnResolution,
    label: &str,
) {
    let ids = seat_ids(state);
    for seat in board.seats() {
        let id = &ids[seat.index()];
        let expected = reference.snake(id).expect("snake is resolved");
        let body: Vec<Cell> = board.serpent(seat).body().collect();
        assert_eq!(
            body,
            resolved_body(reference, id),
            "{label}: seat {seat:?} body"
        );
        assert_eq!(
            board.serpent(seat).vigor(),
            expected.health(),
            "{label}: seat {seat:?} health"
        );
    }
    assert_eq!(
        board.pellets(),
        CellSet::from_bits(reference.food().bits()),
        "{label}: pellets"
    );
}

fn assert_agrees_with_reference(state: &TurnState, by_seat: &[Heading]) {
    let board = ingest_melee(state).expect("a melee converts");
    let next = continued(&board, by_seat);
    let reference = reference_after_all(state, &headings_in_state_order(state, by_seat));

    assert_board_matches(&next, state, &reference, &format!("{by_seat:?}"));
}

#[test]
fn ingest_puts_our_snake_first_and_keeps_the_others_in_order() {
    // Test snakes start at (5,5), (2,2), (9,9), (1,9); "you" is the third.
    let board = ingest_melee(&turn_state(4, 2, &[])).expect("four snakes convert");

    let heads: Vec<Cell> = board
        .seats()
        .map(|seat| board.serpent(seat).head())
        .collect();
    assert_eq!(heads, vec![cell(9, 9), cell(5, 5), cell(2, 2), cell(1, 9)]);
    assert_eq!(board.alive_count(), 4);
}

#[test]
fn ingest_refuses_anything_but_three_or_four_snakes() {
    assert_eq!(
        ingest_melee(&turn_state(2, 0, &[])).unwrap_err(),
        IngestError::NotAMelee
    );
    assert!(ingest_melee(&turn_state(3, 1, &[])).is_ok());
}

#[test]
fn ordinary_joint_moves_of_three_and_four_snakes_match_the_reference() {
    // Every test snake faces north with its body below: north, east and west are open.
    let open = [Heading::North, Heading::East, Heading::West];
    for snakes in [3, 4] {
        for you in 0..snakes {
            let state = turn_state(snakes, you, &[]);
            for first in open {
                for rest in open {
                    let by_seat: Vec<Heading> = std::iter::once(first)
                        .chain(std::iter::repeat_n(rest, snakes - 1))
                        .collect();
                    assert_agrees_with_reference(&state, &by_seat);
                }
            }
        }
    }
}

#[test]
fn a_move_shifts_every_body_and_drops_every_vigor_by_one() {
    let board = ingest_melee(&turn_state(3, 0, &[])).expect("converts");

    let next = continued(&board, &[Heading::North, Heading::West, Heading::East]);

    let us: Vec<Cell> = next.serpent(Seat::US).body().collect();
    let third: Vec<Cell> = next.serpent(Seat::ALL[2]).body().collect();
    assert_eq!(us, vec![cell(5, 6), cell(5, 5), cell(5, 4)]);
    assert_eq!(third, vec![cell(10, 9), cell(9, 9), cell(9, 8)]);
    assert!(next.seats().all(|seat| next.serpent(seat).vigor() == 89));
    assert!(!next.occupied().contains(cell(5, 3)));
    assert_eq!(next.ply(), 1);
}

#[test]
fn several_heads_eat_in_the_same_turn_and_each_pellet_goes_once() {
    let us: &[(i32, i32)] = &[(5, 5), (5, 4), (5, 3)];
    let east: &[(i32, i32)] = &[(9, 9), (9, 8), (9, 7)];
    let west: &[(i32, i32)] = &[(1, 9), (1, 8), (1, 7)];
    let state = turn_state_from_bodies(
        &[us, east, west],
        &[60, 40, 5],
        0,
        &[(5, 6), (9, 10), (1, 10)],
    );

    assert_agrees_with_reference(&state, &[Heading::North, Heading::North, Heading::North]);
    let next = continued(
        &ingest_melee(&state).expect("converts"),
        &[Heading::North; 3],
    );
    assert_eq!(next.pellets(), CellSet::EMPTY);
    assert!(next.seats().all(|seat| next.serpent(seat).vigor() == 100));
    assert!(next.seats().all(|seat| next.serpent(seat).length() == 4));
}

const US: &[(i32, i32)] = &[(5, 5), (5, 4), (5, 3)];
const FAR_EAST: &[(i32, i32)] = &[(9, 9), (9, 8), (9, 7)];
const FAR_WEST: &[(i32, i32)] = &[(1, 9), (1, 8), (1, 7)];

#[test]
fn three_heads_on_one_cell_only_a_strictly_longest_survives() {
    // All three heads step into (5,6): from below (us), from the east, from the west.
    let east: &[(i32, i32)] = &[(6, 6), (7, 6), (8, 6)];
    let west: &[(i32, i32)] = &[(4, 6), (3, 6), (2, 6)];
    let longer_us: &[(i32, i32)] = &[(5, 5), (5, 4), (5, 3), (5, 2)];
    let all_equal = turn_state_from_bodies(&[US, east, west], &[90, 90, 90], 0, &[]);
    let we_are_longest = turn_state_from_bodies(&[longer_us, east, west], &[90, 90, 90], 0, &[]);
    let into = [Heading::North, Heading::West, Heading::East];

    assert_eq!(
        reference_survivors(&all_equal, &into),
        vec![false, false, false]
    );
    assert_outcome_matches(&all_equal, &into);
    assert_eq!(
        reference_survivors(&we_are_longest, &into),
        vec![true, false, false]
    );
    assert_outcome_matches(&we_are_longest, &into);
    assert!(matches!(
        ingest_melee(&we_are_longest)
            .unwrap()
            .advance(&moves(&into)),
        MeleeOutcome::WeAlone
    ));
}

#[test]
fn two_equal_longest_heads_both_die_and_a_shorter_third_dies_too() {
    let east: &[(i32, i32)] = &[(6, 6), (7, 6), (8, 6), (9, 6)];
    let west: &[(i32, i32)] = &[(4, 6), (3, 6), (2, 6), (1, 6)];
    let state = turn_state_from_bodies(&[US, east, west, FAR_EAST], &[90, 90, 90, 90], 0, &[]);
    let into = [Heading::North, Heading::West, Heading::East, Heading::North];

    assert_eq!(
        reference_survivors(&state, &into),
        vec![false, false, false, true]
    );
    assert_outcome_matches(&state, &into);
    assert!(matches!(
        ingest_melee(&state).unwrap().advance(&moves(&into)),
        MeleeOutcome::WeDown { rivals_left: 1 }
    ));
}

#[test]
fn a_dead_seat_is_removed_and_its_cells_free_next_turn() {
    let wall: &[(i32, i32)] = &[(6, 5), (7, 5), (8, 5)];
    let state = turn_state_from_bodies(&[US, wall, FAR_WEST], &[90, 90, 90], 0, &[]);
    // The wall serpent runs into our neck at (5,5); we and the far one live on.
    let by_seat = [Heading::North, Heading::West, Heading::North];

    assert_eq!(
        reference_survivors(&state, &by_seat),
        vec![true, false, true]
    );
    let next = continued(&ingest_melee(&state).unwrap(), &by_seat);
    assert_eq!(next.alive_count(), 2);
    assert!(!next.is_alive(Seat::ALL[1]));
    assert!(!next.occupied().contains(cell(7, 5)));
    assert_outcome_matches(&state, &by_seat);
}

#[test]
fn a_seat_leaving_the_board_still_blocks_with_its_moved_body() {
    let leaver: &[(i32, i32)] = &[(0, 5), (1, 5), (2, 5)];
    let hits_neck: &[(i32, i32)] = &[(1, 6), (1, 7), (1, 8)];
    let state = turn_state_from_bodies(&[hits_neck, leaver, FAR_EAST], &[90, 90, 90], 0, &[]);
    let by_seat = [Heading::South, Heading::West, Heading::North];

    assert_eq!(
        reference_survivors(&state, &by_seat),
        vec![false, false, true]
    );
    assert_outcome_matches(&state, &by_seat);
}

#[test]
fn starving_and_body_collisions_end_seats_like_the_reference() {
    let starving = turn_state_from_bodies(&[US, FAR_EAST, FAR_WEST], &[1, 90, 90], 0, &[]);
    let curled: &[(i32, i32)] = &[(5, 5), (5, 4), (6, 4), (6, 5), (6, 6)];
    let self_hit = turn_state_from_bodies(&[curled, FAR_EAST, FAR_WEST], &[90, 90, 90], 0, &[]);

    assert_outcome_matches(&starving, &[Heading::North, Heading::North, Heading::North]);
    assert_outcome_matches(&self_hit, &[Heading::East, Heading::North, Heading::North]);
    assert!(matches!(
        ingest_melee(&starving)
            .unwrap()
            .advance(&moves(&[Heading::North; 3])),
        MeleeOutcome::WeDown { rivals_left: 2 }
    ));
}

#[test]
fn everyone_dying_at_once_is_a_wipe_out_with_no_rivals_left() {
    let corner_a: &[(i32, i32)] = &[(0, 5), (1, 5), (2, 5)];
    let corner_b: &[(i32, i32)] = &[(10, 5), (9, 5), (8, 5)];
    let corner_c: &[(i32, i32)] = &[(5, 0), (5, 1), (5, 2)];
    let state = turn_state_from_bodies(&[corner_a, corner_b, corner_c], &[90, 90, 90], 0, &[]);
    let out = [Heading::West, Heading::East, Heading::South];

    assert_outcome_matches(&state, &out);
    assert!(matches!(
        ingest_melee(&state).unwrap().advance(&moves(&out)),
        MeleeOutcome::WeDown { rivals_left: 0 }
    ));
}

/// Every joint move of a three-snake position, and a fixed sample of a four-snake one.
fn joint_moves(seats: usize) -> Vec<Vec<Heading>> {
    let total = 4usize.pow(seats as u32);
    let stride = if seats == 4 { 5 } else { 1 };
    (0..total)
        .step_by(stride)
        .map(|code| {
            (0..seats)
                .map(|seat| Heading::ALL[(code >> (2 * seat)) & 3])
                .collect()
        })
        .collect()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    #[test]
    fn the_kernel_agrees_with_the_reference_on_joint_moves_of_generated_melees(
        spec in melee_spec(),
    ) {
        let state = realize_melee(&spec);
        for by_seat in joint_moves(spec.snakes.len()) {
            assert_outcome_matches(&state, &by_seat);
        }
    }
}
