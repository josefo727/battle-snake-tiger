mod support;

use proptest::prelude::*;
use tiger_engine::arena::cellset::{Cell, CellSet};
use tiger_engine::arena::duel::{Advance, DuelBoard, Side, Verdict};
use tiger_engine::arena::heading::Heading;
use tiger_engine::arena::ingest::ingest;
use tiger_engine::rules_core::TurnState;

use support::{
    realize, reference_after, resolved_body, state_after, state_spec, turn_state,
    turn_state_from_bodies,
};

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
fn two_serpents_each_eating_their_own_pellet_both_grow() {
    let state = turn_state_from_bodies(&[US_BODY, THEM_BODY], &[60, 40], 0, &[(5, 6), (1, 2)]);

    assert_agrees_with_reference(&state, Heading::North, Heading::West);
}

/// The verdict the reused resolver implies for a duel: `None` while both
/// serpents survive.
fn reference_verdict(state: &TurnState, us: Heading, them: Heading) -> Option<Verdict> {
    let reference = reference_after(state, us, them);
    let our_id = state.you().id().as_str();
    let their_id = state.snakes()[1 - state.you_index()].id().as_str();
    let we_live = reference.snake(our_id).expect("resolved").survived();
    let they_live = reference.snake(their_id).expect("resolved").survived();
    match (we_live, they_live) {
        (true, true) => None,
        (true, false) => Some(Verdict::WeOnly),
        (false, true) => Some(Verdict::TheyOnly),
        (false, false) => Some(Verdict::BothDown),
    }
}

/// Asserts the kernel ends the duel exactly when and how the reference does.
fn assert_verdict_matches(state: &TurnState, us: Heading, them: Heading) {
    let board = ingest(state).expect("a duel converts");
    let expected = reference_verdict(state, us, them);

    match (board.advance(us, them), expected) {
        (Advance::Continues(next), None) => {
            let reference = reference_after(state, us, them);
            assert_board_matches(&next, state, &reference, &format!("{us:?}/{them:?}"));
        }
        (Advance::Over(got), Some(want)) => {
            assert_eq!(got, want, "{us:?}/{them:?}");
        }
        (got, want) => panic!("{us:?}/{them:?}: kernel gave {got:?}, reference implies {want:?}"),
    }
}

const FAR: &[(i32, i32)] = &[(8, 8), (8, 9), (8, 10)];

#[test]
fn a_serpent_at_vigor_one_starves_unless_it_eats() {
    let starving = turn_state_from_bodies(&[US_BODY, FAR], &[1, 90], 0, &[]);
    let fed = turn_state_from_bodies(&[US_BODY, FAR], &[1, 90], 0, &[(5, 6)]);
    let both = turn_state_from_bodies(&[US_BODY, FAR], &[1, 1], 0, &[]);

    assert_eq!(
        reference_verdict(&starving, Heading::North, Heading::East),
        Some(Verdict::TheyOnly)
    );
    assert_verdict_matches(&starving, Heading::North, Heading::East);
    assert_verdict_matches(&fed, Heading::North, Heading::East);
    assert_verdict_matches(&both, Heading::North, Heading::East);
}

#[test]
fn leaving_the_board_eliminates_and_two_exits_end_in_a_double_loss() {
    let corner_us: &[(i32, i32)] = &[(0, 5), (1, 5), (2, 5)];
    let corner_them: &[(i32, i32)] = &[(10, 5), (9, 5), (8, 5)];
    let state = turn_state_from_bodies(&[corner_us, corner_them], &[90, 90], 0, &[]);

    assert_verdict_matches(&state, Heading::West, Heading::North);
    assert_verdict_matches(&state, Heading::North, Heading::East);
    assert_verdict_matches(&state, Heading::West, Heading::East);
    assert_verdict_matches(&state, Heading::South, Heading::North);
}

#[test]
fn a_head_hitting_its_own_body_is_eliminated_but_its_vacating_tail_is_safe() {
    let curled: &[(i32, i32)] = &[(5, 5), (5, 4), (6, 4), (6, 5), (6, 6)];
    let looped: &[(i32, i32)] = &[(1, 1), (1, 0), (0, 0), (0, 1)];
    let stacked: &[(i32, i32)] = &[(1, 1), (1, 0), (0, 0), (0, 1), (0, 1)];
    let hits_body = turn_state_from_bodies(&[curled, FAR], &[90, 90], 0, &[]);
    let hits_tail = turn_state_from_bodies(&[looped, FAR], &[90, 90], 0, &[]);
    let hits_stack = turn_state_from_bodies(&[stacked, FAR], &[90, 90], 0, &[]);

    assert_verdict_matches(&hits_body, Heading::East, Heading::East);
    assert_verdict_matches(&hits_tail, Heading::West, Heading::East);
    assert_verdict_matches(&hits_stack, Heading::West, Heading::East);
}

#[test]
fn entering_the_other_serpents_body_eliminates_but_its_vacating_tail_does_not() {
    let wall: &[(i32, i32)] = &[(6, 6), (6, 5), (6, 4)];
    let stacked_wall: &[(i32, i32)] = &[(6, 6), (6, 5), (6, 4), (6, 4)];
    let side: &[(i32, i32)] = &[(7, 4), (8, 4), (9, 4)];
    let into_body = turn_state_from_bodies(&[US_BODY, wall], &[90, 90], 0, &[]);
    let into_tail = turn_state_from_bodies(&[side, wall], &[90, 90], 0, &[]);
    let into_stack = turn_state_from_bodies(&[side, stacked_wall], &[90, 90], 0, &[]);

    assert_verdict_matches(&into_body, Heading::East, Heading::North);
    assert_verdict_matches(&into_tail, Heading::West, Heading::North);
    assert_verdict_matches(&into_stack, Heading::West, Heading::North);
}

#[test]
fn a_serpent_leaving_the_board_still_blocks_with_its_moved_body() {
    let leaver: &[(i32, i32)] = &[(0, 5), (1, 5), (2, 5)];
    let hits_neck: &[(i32, i32)] = &[(1, 6), (1, 7), (1, 8)];
    let enters_old_tail: &[(i32, i32)] = &[(2, 6), (2, 7), (2, 8)];
    let neck = turn_state_from_bodies(&[hits_neck, leaver], &[90, 90], 0, &[]);
    let tail = turn_state_from_bodies(&[enters_old_tail, leaver], &[90, 90], 0, &[]);

    assert_eq!(
        reference_verdict(&neck, Heading::South, Heading::West),
        Some(Verdict::BothDown)
    );
    assert_verdict_matches(&neck, Heading::South, Heading::West);
    assert_verdict_matches(&tail, Heading::South, Heading::West);
}

#[test]
fn surviving_joint_moves_still_agree_with_the_reference_after_the_new_checks() {
    let state = turn_state(2, 0, &[]);

    assert_verdict_matches(&state, Heading::North, Heading::North);
    assert_verdict_matches(&state, Heading::East, Heading::West);
}

#[test]
fn head_to_head_is_won_only_by_the_strictly_longer_serpent() {
    let long: &[(i32, i32)] = &[(5, 5), (5, 4), (5, 3), (5, 2)];
    let short: &[(i32, i32)] = &[(5, 7), (5, 8), (5, 9)];
    let equal: &[(i32, i32)] = &[(5, 7), (5, 8), (5, 9), (5, 10)];
    let we_win = turn_state_from_bodies(&[long, short], &[90, 90], 0, &[]);
    let they_win = turn_state_from_bodies(&[short_mirror(), long_mirror()], &[90, 90], 0, &[]);
    let tie = turn_state_from_bodies(&[long, equal], &[90, 90], 0, &[]);

    assert_eq!(
        reference_verdict(&we_win, Heading::North, Heading::South),
        Some(Verdict::WeOnly)
    );
    assert_verdict_matches(&we_win, Heading::North, Heading::South);
    assert_eq!(
        reference_verdict(&they_win, Heading::North, Heading::South),
        Some(Verdict::TheyOnly)
    );
    assert_verdict_matches(&they_win, Heading::North, Heading::South);
    assert_eq!(
        reference_verdict(&tie, Heading::North, Heading::South),
        Some(Verdict::BothDown)
    );
    assert_verdict_matches(&tie, Heading::North, Heading::South);
}

fn short_mirror() -> &'static [(i32, i32)] {
    &[(5, 5), (5, 4), (5, 3)]
}

fn long_mirror() -> &'static [(i32, i32)] {
    &[(5, 7), (5, 8), (5, 9), (5, 10)]
}

#[test]
fn head_to_head_on_a_pellet_compares_lengths_after_both_have_grown() {
    let long: &[(i32, i32)] = &[(5, 5), (5, 4), (5, 3), (5, 2)];
    let short: &[(i32, i32)] = &[(5, 7), (5, 8), (5, 9)];
    let same: &[(i32, i32)] = &[(5, 7), (5, 8), (5, 9), (5, 10)];
    let longer_eats = turn_state_from_bodies(&[long, short], &[90, 90], 0, &[(5, 6)]);
    let equal_eats = turn_state_from_bodies(&[long, same], &[90, 90], 0, &[(5, 6)]);

    assert_verdict_matches(&longer_eats, Heading::North, Heading::South);
    assert_verdict_matches(&equal_eats, Heading::North, Heading::South);
}

#[test]
fn a_head_to_head_winner_that_starves_still_ends_in_a_double_loss() {
    let long: &[(i32, i32)] = &[(5, 5), (5, 4), (5, 3), (5, 2)];
    let short: &[(i32, i32)] = &[(5, 7), (5, 8), (5, 9)];
    let state = turn_state_from_bodies(&[long, short], &[1, 90], 0, &[]);

    assert_eq!(
        reference_verdict(&state, Heading::North, Heading::South),
        Some(Verdict::BothDown)
    );
    assert_verdict_matches(&state, Heading::North, Heading::South);
}

#[test]
fn heads_swapping_places_collide_with_each_others_old_head() {
    let west: &[(i32, i32)] = &[(5, 5), (4, 5), (3, 5)];
    let east: &[(i32, i32)] = &[(6, 5), (7, 5), (8, 5)];
    let state = turn_state_from_bodies(&[west, east], &[90, 90], 0, &[]);

    assert_eq!(
        reference_verdict(&state, Heading::East, Heading::West),
        Some(Verdict::BothDown)
    );
    assert_verdict_matches(&state, Heading::East, Heading::West);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(400))]

    #[test]
    fn the_kernel_agrees_with_the_reference_on_every_joint_move_of_generated_duels(
        spec in state_spec(),
    ) {
        let state = realize(&spec);
        for us in Heading::ALL {
            for them in Heading::ALL {
                assert_verdict_matches(&state, us, them);
            }
        }
    }
}

#[test]
fn two_serpents_entering_the_same_pellet_cell_are_settled_by_length() {
    let above: &[(i32, i32)] = &[(5, 7), (5, 8), (5, 9)];
    let state = turn_state_from_bodies(&[US_BODY, above], &[60, 40], 0, &[(5, 6)]);

    assert_eq!(
        reference_verdict(&state, Heading::North, Heading::South),
        Some(Verdict::BothDown)
    );
    assert_verdict_matches(&state, Heading::North, Heading::South);
}
