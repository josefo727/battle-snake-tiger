mod support;

use tiger_engine::arena::cellset::{Cell, CellSet};
use tiger_engine::arena::duel::{Advance, DuelBoard, Side};
use tiger_engine::arena::heading::Heading;
use tiger_engine::arena::ingest::ingest;
use tiger_engine::rules_core::TurnState;

use support::{reference_after, resolved_body, state_after, turn_state, turn_state_from_bodies};

fn cell(x: u8, y: u8) -> Cell {
    Cell::from_xy(x, y).expect("test coordinate is on the board")
}

fn cells(coordinates: &[(u8, u8)]) -> Vec<Cell> {
    coordinates.iter().map(|&(x, y)| cell(x, y)).collect()
}

fn continued(board: &DuelBoard, us: Heading, them: Heading) -> DuelBoard {
    match board.advance(us, them) {
        Advance::Continues(next) => next,
        Advance::Over(verdict) => panic!("the game unexpectedly ended: {verdict:?}"),
    }
}

/// Asserts that a kernel board and the reused resolver's answer agree on both
/// bodies, both health values, and the pellets left on the board.
fn assert_board_matches(
    board: &DuelBoard,
    state: &TurnState,
    reference: &tiger_engine::rules_core::TurnResolution,
    label: &str,
) {
    let our_id = state.you().id().as_str();
    let their_id = state.snakes()[1 - state.you_index()].id().as_str();
    for (side, id) in [(Side::Us, our_id), (Side::Them, their_id)] {
        let serpent = board.serpent(side);
        let expected = reference.snake(id).expect("snake is resolved");
        let body: Vec<Cell> = serpent.body().collect();
        assert_eq!(body, resolved_body(reference, id), "{label}: {side:?} body");
        assert_eq!(
            serpent.vigor(),
            expected.health(),
            "{label}: {side:?} health"
        );
    }
    assert_eq!(
        board.pellets(),
        CellSet::from_bits(reference.food().bits()),
        "{label}: pellets"
    );
}

/// Asserts agreement after one joint move.
fn assert_agrees_with_reference(state: &TurnState, us: Heading, them: Heading) {
    let board = ingest(state).expect("a duel converts");
    let next = continued(&board, us, them);
    let reference = reference_after(state, us, them);

    assert_board_matches(&next, state, &reference, &format!("{us:?}/{them:?}"));
}

/// Advances the kernel and the reference in lockstep, asserting agreement after
/// every joint move and feeding the reference's result back in as the next state.
fn assert_sequence_agrees(start: TurnState, moves: &[(Heading, Heading)]) -> Vec<DuelBoard> {
    let mut state = start;
    let mut board = ingest(&state).expect("a duel converts");
    let mut boards = Vec::new();
    for (step, &(us, them)) in moves.iter().enumerate() {
        board = continued(&board, us, them);
        let reference = reference_after(&state, us, them);
        assert_board_matches(&board, &state, &reference, &format!("step {step}"));
        state = state_after(&state, &reference);
        boards.push(board);
    }
    boards
}

#[test]
fn ordinary_joint_moves_match_the_reference_for_every_non_colliding_pair() {
    // Us at (5,5) facing up with the body below and them at (2,2) likewise:
    // north, east, and west are open for both; south would hit their own body.
    let state = turn_state(2, 0, &[]);
    let open = [Heading::North, Heading::East, Heading::West];

    for us in open {
        for them in open {
            assert_agrees_with_reference(&state, us, them);
        }
    }
}

#[test]
fn a_move_shifts_the_body_drops_health_by_one_and_releases_the_old_tail() {
    let board = ingest(&turn_state(2, 0, &[])).expect("a duel converts");

    let next = continued(&board, Heading::North, Heading::West);

    let us: Vec<Cell> = next.serpent(Side::Us).body().collect();
    let them: Vec<Cell> = next.serpent(Side::Them).body().collect();
    assert_eq!(us, cells(&[(5, 6), (5, 5), (5, 4)]));
    assert_eq!(them, cells(&[(1, 2), (2, 2), (2, 1)]));
    assert_eq!(next.serpent(Side::Us).vigor(), 89);
    assert_eq!(next.serpent(Side::Them).vigor(), 89);
    assert!(!next.occupied().contains(cell(5, 3)));
    assert!(!next.occupied().contains(cell(2, 0)));
    assert_eq!(next.ply(), 1);
}

#[test]
fn a_head_may_enter_the_cell_its_own_tail_is_vacating() {
    let looped: &[(i32, i32)] = &[(1, 1), (1, 0), (0, 0), (0, 1)];
    let far: &[(i32, i32)] = &[(8, 8), (8, 9), (8, 10)];
    let state = turn_state_from_bodies(&[looped, far], &[50, 50], 0, &[]);

    assert_agrees_with_reference(&state, Heading::West, Heading::East);
}

#[test]
fn both_serpents_move_from_the_same_starting_board() {
    let board = ingest(&turn_state(2, 0, &[])).expect("a duel converts");

    let after_north = continued(&board, Heading::North, Heading::North);
    let after_east = continued(&board, Heading::North, Heading::East);

    let us_north: Vec<Cell> = after_north.serpent(Side::Us).body().collect();
    let us_east: Vec<Cell> = after_east.serpent(Side::Us).body().collect();
    assert_eq!(
        us_north, us_east,
        "our result must not depend on their heading"
    );
}

const US_BODY: &[(i32, i32)] = &[(5, 5), (5, 4), (5, 3)];
const THEM_BODY: &[(i32, i32)] = &[(2, 2), (2, 1), (2, 0)];

#[test]
fn eating_restores_vigor_removes_the_pellet_and_stacks_the_current_tail() {
    let state = turn_state_from_bodies(&[US_BODY, THEM_BODY], &[90, 90], 0, &[(5, 6)]);
    let board = ingest(&state).expect("a duel converts");

    let next = continued(&board, Heading::North, Heading::West);

    let us: Vec<Cell> = next.serpent(Side::Us).body().collect();
    assert_eq!(us, cells(&[(5, 6), (5, 5), (5, 4), (5, 4)]));
    assert_eq!(next.serpent(Side::Us).vigor(), 100);
    assert_eq!(next.pellets(), CellSet::EMPTY);
    assert_agrees_with_reference(&state, Heading::North, Heading::West);
}

#[test]
fn eating_at_the_brink_of_starvation_resets_vigor_to_the_maximum() {
    let state = turn_state_from_bodies(&[US_BODY, THEM_BODY], &[1, 90], 0, &[(6, 5)]);

    assert_agrees_with_reference(&state, Heading::East, Heading::West);
}

#[test]
fn a_stacked_tail_releases_one_copy_per_move_over_three_turns() {
    let state = turn_state_from_bodies(&[US_BODY, THEM_BODY], &[90, 90], 0, &[(5, 6)]);

    let boards = assert_sequence_agrees(
        state,
        &[
            (Heading::North, Heading::North),
            (Heading::North, Heading::North),
            (Heading::North, Heading::North),
        ],
    );

    assert!(
        boards[1].occupied().contains(cell(5, 4)),
        "one copy of the stack remains"
    );
    assert!(
        !boards[2].occupied().contains(cell(5, 4)),
        "the second copy is released"
    );
}

#[test]
fn two_serpents_entering_the_same_pellet_cell_both_eat() {
    let above: &[(i32, i32)] = &[(5, 7), (5, 8), (5, 9)];
    let state = turn_state_from_bodies(&[US_BODY, above], &[60, 40], 0, &[(5, 6)]);

    assert_agrees_with_reference(&state, Heading::North, Heading::South);
}
