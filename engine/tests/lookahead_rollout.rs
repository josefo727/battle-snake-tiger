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
