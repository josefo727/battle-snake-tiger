mod support;

use tiger_engine::arena::cellset::{Cell, CellSet};
use tiger_engine::arena::heading::Heading;
use tiger_engine::arena::ingest::{IngestError, ingest_melee};
use tiger_engine::arena::melee::{MAX_SEATS, MeleeBoard, MeleeOutcome, Seat};
use tiger_engine::rules_core::{TurnResolution, TurnState};

use support::{
    headings_in_state_order, reference_after_all, resolved_body, seat_ids, turn_state,
    turn_state_from_bodies,
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
