mod support;

use tiger_engine::arena::ingest::ingest_melee;
use tiger_engine::arena::melee::{MeleeBoard, Seat};
use tiger_engine::valuation::Assessor;
use tiger_engine::valuation::melee::Surveyed;
use tiger_engine::valuation::melee::appetite::{Appetite, REACH};
use tiger_engine::valuation::melee::craving::{Craving, turns_to_food};

use support::turn_state_from_bodies;

fn melee(bodies: &[&[(i32, i32)]], health: &[i32], food: &[(i32, i32)]) -> MeleeBoard {
    let state = turn_state_from_bodies(bodies, health, 0, food);
    ingest_melee(&state).expect("a melee converts")
}

#[test]
fn a_pellet_a_rival_reaches_first_still_pulls() {
    // The whole reason the term exists. The pellet at (5,5) is one step from
    // the rival's head and three from ours, so the survey grants it to the
    // rival and appetite, larder and hunger all fall silent: the engine has no
    // reason left to go anywhere near the food. Craving still pulls.
    let us: &[(i32, i32)] = &[(5, 2), (5, 1), (5, 0)];
    let rival: &[(i32, i32)] = &[(5, 6), (5, 7), (5, 8), (5, 9)];
    let third: &[(i32, i32)] = &[(0, 0), (0, 1), (0, 2)];
    let board = melee(&[us, rival, third], &[90, 90, 90], &[(5, 5)]);
    let position = Surveyed::new(&board);

    assert_eq!(
        Appetite.assess(&position),
        0,
        "the pellet is the rival's, so the owned-food term says nothing"
    );
    assert_eq!(turns_to_food(&board, Seat::US), Some(3));
    assert_eq!(Craving.assess(&position), REACH - 3);
}

#[test]
fn the_pull_grows_as_the_pellet_comes_nearer() {
    let far: &[(i32, i32)] = &[(5, 0), (5, 1), (5, 2)];
    let near: &[(i32, i32)] = &[(5, 3), (5, 2), (5, 1)];
    let rival: &[(i32, i32)] = &[(0, 10), (1, 10), (2, 10)];
    let third: &[(i32, i32)] = &[(10, 0), (10, 1), (10, 2)];

    let from_far = melee(&[far, rival, third], &[90, 90, 90], &[(5, 6)]);
    let from_near = melee(&[near, rival, third], &[90, 90, 90], &[(5, 6)]);

    assert!(
        Craving.assess(&Surveyed::new(&from_near)) > Craving.assess(&Surveyed::new(&from_far)),
        "three steps away is worth more than six"
    );
}

#[test]
fn a_rival_body_is_a_wall_and_our_own_is_not() {
    // The pellet sits behind a rival's body: unreachable, so no pull. The same
    // pellet behind our own body is reachable, because our tail gives the way
    // through back one cell at a time.
    let us: &[(i32, i32)] = &[(0, 1), (0, 2), (0, 3)];
    let wall: &[(i32, i32)] = &[
        (0, 4),
        (1, 4),
        (1, 3),
        (1, 2),
        (1, 1),
        (1, 0),
        (2, 0),
        (3, 0),
    ];
    let third: &[(i32, i32)] = &[(9, 9), (9, 8), (9, 7)];
    let shut_out = melee(&[us, wall, third], &[90, 90, 90], &[(5, 5)]);

    assert_eq!(
        turns_to_food(&shut_out, Seat::US),
        None,
        "every way to the pellet runs through a rival"
    );
    assert_eq!(Craving.assess(&Surveyed::new(&shut_out)), 0);

    let ours_in_the_way: &[(i32, i32)] = &[
        (0, 0),
        (0, 1),
        (0, 2),
        (0, 3),
        (1, 3),
        (1, 2),
        (1, 1),
        (1, 0),
        (2, 0),
    ];
    let elsewhere: &[(i32, i32)] = &[(9, 1), (9, 2), (9, 3)];
    let boxed_by_us = melee(
        &[ours_in_the_way, elsewhere, third],
        &[90, 90, 90],
        &[(3, 1)],
    );

    assert!(
        turns_to_food(&boxed_by_us, Seat::US).is_some(),
        "our own body is not a wall: the tail frees and we get out"
    );
}

#[test]
fn nothing_to_reach_is_nothing_to_want() {
    let us: &[(i32, i32)] = &[(5, 5), (5, 4), (5, 3)];
    let rival: &[(i32, i32)] = &[(1, 1), (1, 2), (1, 3)];
    let third: &[(i32, i32)] = &[(9, 9), (9, 8), (9, 7)];
    let bare = melee(&[us, rival, third], &[90, 90, 90], &[]);

    assert_eq!(turns_to_food(&bare, Seat::US), None);
    assert_eq!(Craving.assess(&Surveyed::new(&bare)), 0);
}

#[test]
fn a_pellet_beyond_the_reach_is_worth_nothing_and_costs_no_more_to_say_so() {
    // The term prices anything past REACH at zero, so the fill stops there
    // rather than walking the whole board to find out.
    let us: &[(i32, i32)] = &[(0, 0), (0, 1), (0, 2)];
    let rival: &[(i32, i32)] = &[(5, 5), (5, 6), (5, 7)];
    let third: &[(i32, i32)] = &[(9, 0), (9, 1), (9, 2)];
    let board = melee(&[us, rival, third], &[90, 90, 90], &[(10, 10)]);

    assert_eq!(turns_to_food(&board, Seat::US), None);
    assert_eq!(Craving.assess(&Surveyed::new(&board)), 0);
}
