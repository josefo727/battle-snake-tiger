mod support;

use proptest::prelude::*;
use tiger_engine::arena::ingest::ingest_melee;
use tiger_engine::arena::melee::MeleeBoard;
use tiger_engine::lookahead::allowance::SEARCH_TAIL_MARGIN;
use tiger_engine::lookahead::allowance::SearchAllowance;
use tiger_engine::lookahead::deepening::{IterativeSearch, drive};
use tiger_engine::lookahead::ordering::NaturalOrder;
use tiger_engine::lookahead::parallel::RootSplit;
use tiger_engine::lookahead::paranoid::MeleeSearcher;
use tiger_engine::rules_core::{Clock, MonotonicInstant};
use tiger_engine::valuation::melee::MeleeValuation;
use tiger_engine::valuation::melee::finish::MeleeFinish;
use tiger_engine::valuation::melee::weights::DEFAULT_MELEE_PROFILE;

use support::clock::{ManualClock, ScriptedClock};
use support::{melee_spec, realize_melee, turn_state};

fn finish() -> MeleeFinish {
    MeleeFinish::new(&DEFAULT_MELEE_PROFILE)
}

fn far_deadline() -> MonotonicInstant {
    MonotonicInstant {
        microseconds: 10_000_000_000,
    }
}

/// The split search at a fixed depth must give the sequential search's answer.
fn assert_split_equals_sequential(board: &MeleeBoard, depth: u16, threads: usize) {
    let valuation = MeleeValuation::standard();
    let clock = ManualClock::at_micros(0);
    let sequential =
        MeleeSearcher::with_order(&valuation, finish(), NaturalOrder).search_fixed(board, depth);
    let mut split = RootSplit::new(
        &valuation,
        finish(),
        *board,
        &clock,
        far_deadline(),
        threads,
    );

    let report = split.search_fixed_split(depth).expect("never stopped");

    assert_eq!(
        report.best, sequential.best,
        "{threads} threads, depth {depth}"
    );
    assert_eq!(
        report.principal_score, sequential.principal_score,
        "{threads} threads, depth {depth}"
    );
    assert_eq!(report.completed_depth, depth);
    assert!(report.nodes_explored > 0);
    assert_eq!(split.nodes_visited(), report.nodes_explored);
}

#[test]
fn the_opening_position_is_answered_alike_by_one_two_and_four_threads() {
    let board = ingest_melee(&turn_state(4, 0, &[(5, 6), (2, 3)])).expect("melee");
    for threads in [1, 2, 4] {
        assert_split_equals_sequential(&board, 1, threads);
        assert_split_equals_sequential(&board, 3, threads);
    }
}

#[test]
fn a_lane_that_is_stopped_stops_the_whole_iteration_with_nothing_partial() {
    let board = ingest_melee(&turn_state(4, 0, &[(5, 6)])).expect("melee");
    let valuation = MeleeValuation::standard();
    // Expired from the first read: every lane's first poll stops it.
    let clock = ScriptedClock::expiring_after(0);
    let mut split = RootSplit::new(
        &valuation,
        finish(),
        board,
        &clock,
        MonotonicInstant { microseconds: 1 },
        2,
    );

    assert!(split.search_fixed_split(4).is_none());
}

#[test]
fn the_deepening_driver_drives_the_split_like_a_searcher() {
    let board = ingest_melee(&turn_state(4, 0, &[(5, 6)])).expect("melee");
    let valuation = MeleeValuation::standard();
    let clock = ManualClock::at_micros(0);
    let margin = u64::try_from(SEARCH_TAIL_MARGIN.as_micros()).unwrap();
    let mut allowance = SearchAllowance::new(
        &clock,
        MonotonicInstant {
            microseconds: far_deadline().microseconds + margin,
        },
    );
    let mut split = RootSplit::new(&valuation, finish(), board, &clock, far_deadline(), 2);

    let report = drive(&mut split, &mut allowance, 3);

    let sequential =
        MeleeSearcher::with_order(&valuation, finish(), NaturalOrder).search_fixed(&board, 3);
    assert_eq!(report.completed_depth, 3);
    assert_eq!(report.best, sequential.best);
    assert_eq!(report.principal_score, sequential.principal_score);
    let _ = clock.now();
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(30))]

    #[test]
    fn two_threads_agree_with_the_sequential_search_on_generated_melees(spec in melee_spec()) {
        let board = ingest_melee(&realize_melee(&spec)).expect("melee");
        assert_split_equals_sequential(&board, 1, 2);
        assert_split_equals_sequential(&board, 2, 2);
    }
}
