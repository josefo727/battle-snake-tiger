mod support;

use tiger_engine::arena::cellset::Cell;
use tiger_engine::arena::duel::{Advance, DuelBoard, Side};
use tiger_engine::arena::heading::Heading;
use tiger_engine::arena::ingest::ingest;
use tiger_engine::rules_core::TurnState;

use support::{reference_after, resolved_body, turn_state, turn_state_from_bodies};

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

/// Asserts that the kernel and the reused resolver agree on both bodies and
/// both health values after one joint move.
fn assert_agrees_with_reference(state: &TurnState, us: Heading, them: Heading) {
    let board = ingest(state).expect("a duel converts");
    let next = continued(&board, us, them);
    let reference = reference_after(state, us, them);

    let our_id = state.you().id().as_str();
    let their_id = state.snakes()[1 - state.you_index()].id().as_str();
    for (side, id) in [(Side::Us, our_id), (Side::Them, their_id)] {
        let serpent = next.serpent(side);
        let expected = reference.snake(id).expect("snake is resolved");
        let body: Vec<Cell> = serpent.body().collect();
        assert_eq!(
            body,
            resolved_body(&reference, id),
            "{side:?} body after {us:?}/{them:?}"
        );
        assert_eq!(
            serpent.vigor(),
            expected.health(),
            "{side:?} health after {us:?}/{them:?}"
        );
    }
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
