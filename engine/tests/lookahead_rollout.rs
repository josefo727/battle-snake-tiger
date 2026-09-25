mod support;

use tiger_engine::arena::heading::Heading;
use tiger_engine::arena::ingest::ingest_melee;
use tiger_engine::arena::melee::MeleeBoard;
use tiger_engine::lookahead::allowance::{NeverStop, StopSignal};
use tiger_engine::lookahead::rollout::{ROLLOUTS, best_heading, survival};

use support::turn_state_from_bodies;

fn melee(bodies: &[&[(i32, i32)]], health: &[i32], food: &[(i32, i32)]) -> MeleeBoard {
    let state = turn_state_from_bodies(bodies, health, 0, food);
    ingest_melee(&state).expect("a melee converts")
}

/// Our head at the mouth of a pocket three cells deep that our own body walls
/// off, with the open board one step the other way.
fn pocket() -> MeleeBoard {
    let us: &[(i32, i32)] = &[
        (0, 3),
        (1, 3),
        (1, 2),
        (1, 1),
        (1, 0),
        (2, 0),
        (3, 0),
        (4, 0),
        (5, 0),
    ];
    let rival: &[(i32, i32)] = &[(9, 9), (9, 8), (9, 7)];
    let third: &[(i32, i32)] = &[(9, 1), (9, 2), (9, 3)];
    melee(&[us, rival, third], &[90, 90, 90], &[])
}

#[test]
fn the_pocket_is_worth_fewer_turns_than_the_open_board() {
    let board = pocket();

    let into_the_pocket = survival(&board, Heading::South, &mut NeverStop).expect("no stop");
    let into_the_open = survival(&board, Heading::North, &mut NeverStop).expect("no stop");

    assert!(
        into_the_pocket < 5 * ROLLOUTS,
        "the pocket is four turns long however the rest would have gone (got {into_the_pocket})"
    );
    assert!(
        into_the_open > into_the_pocket,
        "and the open board is worth more (got {into_the_open})"
    );
    assert_eq!(best_heading(&board, &mut NeverStop), Some(Heading::North));
}

#[test]
fn the_same_position_gives_the_same_answer_every_time() {
    // A decision nobody can reproduce is one nobody can argue with afterwards.
    let board = pocket();
    assert_eq!(
        survival(&board, Heading::North, &mut NeverStop),
        survival(&board, Heading::North, &mut NeverStop)
    );
    assert_eq!(
        best_heading(&board, &mut NeverStop),
        best_heading(&board, &mut NeverStop)
    );
}

#[test]
fn a_heading_that_dies_at_once_is_worth_nothing() {
    // West leaves the board. Every rollout ends on the turn it starts.
    let board = pocket();
    assert_eq!(survival(&board, Heading::West, &mut NeverStop), Some(0));
}

/// Says stop once `budget` questions have been asked.
struct StopAfter {
    budget: u64,
    asked: u64,
}

impl StopSignal for StopAfter {
    fn should_stop(&mut self) -> bool {
        self.asked += 1;
        self.asked > self.budget
    }
}

#[test]
fn the_stop_signal_is_obeyed_and_nothing_partial_comes_back() {
    // The rescue runs on what is left of a turn's budget, so it has to be able
    // to give up, and an answer from half the rollouts would not be the same
    // answer twice.
    let board = pocket();

    let mut immediately = StopAfter {
        budget: 0,
        asked: 0,
    };
    assert_eq!(survival(&board, Heading::North, &mut immediately), None);

    let mut midway = StopAfter {
        budget: u64::from(ROLLOUTS) + 10,
        asked: 0,
    };
    assert_eq!(best_heading(&board, &mut midway), None);
}

#[test]
fn a_rival_that_can_take_our_head_for_free_takes_it() {
    // The flaw that cost a ladder game on 2026-09-25. Our head is one step from
    // a cell a rival four segments longer can also enter. Moving there is
    // certain death: the rival wins the head-to-head and loses nothing. A rival
    // that steps at random only takes it now and then, so the reading came back
    // worth fifteen turns of life and the rescue walked into it.
    //
    // Rivals decline nothing that is free, so the rollout does not either.
    let us: &[(i32, i32)] = &[(1, 8), (1, 9), (1, 10), (2, 10), (3, 10), (3, 9)];
    let bigger: &[(i32, i32)] = &[
        (2, 7),
        (3, 7),
        (3, 8),
        (4, 8),
        (5, 8),
        (5, 7),
        (5, 6),
        (4, 6),
        (3, 6),
        (2, 6),
    ];
    let far: &[(i32, i32)] = &[(5, 4), (5, 3), (4, 3), (3, 3), (2, 3), (1, 3)];
    let board = melee(&[us, bigger, far], &[90, 90, 90], &[(0, 10)]);

    // East is (2,8), which the longer rival's head at (2,7) reaches as well.
    let into_the_jaws = survival(&board, Heading::East, &mut NeverStop).expect("no stop");
    assert_eq!(
        into_the_jaws, 0,
        "a cell a longer rival can take is worth no turns at all"
    );
    assert_ne!(
        best_heading(&board, &mut NeverStop),
        Some(Heading::East),
        "and the rescue does not choose it"
    );
}
