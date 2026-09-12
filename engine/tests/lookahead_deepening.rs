mod support;

use proptest::prelude::*;
use tiger_engine::arena::duel::DuelBoard;
use tiger_engine::arena::ingest::ingest;
use tiger_engine::lookahead::allowance::{
    NeverStop, SEARCH_TAIL_MARGIN, SearchAllowance, StopSignal,
};
use tiger_engine::lookahead::deepening::{ITERATION_START_PERCENT, deepen};
use tiger_engine::lookahead::ledger::LookaheadReport;
use tiger_engine::lookahead::minimax::Searcher;
use tiger_engine::lookahead::ordering::NaturalOrder;
use tiger_engine::rules_core::{Clock, MonotonicInstant};
use tiger_engine::valuation::finish::Finish;
use tiger_engine::valuation::leverage::LengthAdvantage;
use tiger_engine::valuation::weights::DEFAULT_PROFILE;
use tiger_engine::valuation::{AssessorSet, ValuationPipeline};

use support::clock::{ManualClock, ScriptedClock};
use support::{realize, state_spec, turn_state, turn_state_from_bodies};

/// The allowance whose search deadline (after the tail margin) is `search_deadline`.
fn allowance_ending_at(clock: &dyn Clock, search_deadline: u64) -> SearchAllowance<'_> {
    let margin = u64::try_from(SEARCH_TAIL_MARGIN.as_micros()).unwrap();
    SearchAllowance::new(
        clock,
        MonotonicInstant {
            microseconds: search_deadline + margin,
        },
    )
}

fn finish() -> Finish {
    Finish::new(&DEFAULT_PROFILE)
}

fn start() -> DuelBoard {
    ingest(&turn_state(2, 0, &[(10, 10)])).expect("a duel converts")
}

/// What a plain fixed-depth search says, the yardstick for every driver result.
fn fixed<S: AssessorSet>(
    pipeline: &ValuationPipeline<S>,
    board: &DuelBoard,
    depth: u16,
) -> LookaheadReport {
    Searcher::with_order(pipeline, finish(), NaturalOrder).search_fixed(board, depth)
}

fn decided(report: &LookaheadReport) -> bool {
    report
        .principal_score
        .is_some_and(|s| s.abs() >= finish().finite_limit())
}

const SEARCH_DEADLINE: u64 = 1_000_000;

// ---- the driver -------------------------------------------------------------

#[test]
fn a_generous_allowance_reaches_the_depth_limit_with_that_depths_answer() {
    let pipeline = ValuationPipeline::standard();
    let board = start();
    let clock = ManualClock::at_micros(0);
    let mut allowance = allowance_ending_at(&clock, 10_000_000_000);
    let mut searcher = Searcher::new(&pipeline, finish());

    let report = deepen(&mut searcher, &board, &mut allowance, 3);

    let expected = fixed(&pipeline, &board, 3);
    assert_eq!(report.completed_depth, 3);
    assert_eq!(report.best, expected.best);
    assert_eq!(report.principal_score, expected.principal_score);
    assert!(report.has_result());
    assert_eq!(report.nodes_explored, searcher.nodes_visited());
    assert!(report.nodes_explored >= expected.nodes_explored / 4);
    assert_eq!(
        clock.reads(),
        3,
        "one boundary read per iteration and nothing mid-way"
    );
}

#[test]
fn a_decided_position_stops_deepening_at_the_depth_that_decides_it() {
    // They have one health left, so they starve this turn whatever they do.
    let us: &[(i32, i32)] = &[(5, 5), (5, 4), (5, 3)];
    let them: &[(i32, i32)] = &[(2, 2), (2, 1), (2, 0)];
    let board = ingest(&turn_state_from_bodies(&[us, them], &[90, 1], 0, &[])).expect("a duel");
    let pipeline = ValuationPipeline::standard();
    let clock = ManualClock::at_micros(0);
    let mut allowance = allowance_ending_at(&clock, 10_000_000_000);

    let report = deepen(
        &mut Searcher::new(&pipeline, finish()),
        &board,
        &mut allowance,
        6,
    );

    assert_eq!(report.completed_depth, 1);
    assert_eq!(
        report.principal_score,
        Some(DEFAULT_PROFILE.win_score - DEFAULT_PROFILE.ply_penalty)
    );
    assert_eq!(clock.reads(), 1);
}

#[test]
fn no_depth_starts_once_the_start_fraction_of_the_allowance_has_elapsed() {
    // Boundary reads: start, 10%, 39%, exactly 40%. Depth 4 must not start.
    let pipeline = ValuationPipeline::standard();
    let board = start();
    let clock = ScriptedClock::new(&[0, 100_000, 390_000, 400_000]);
    let mut allowance = allowance_ending_at(&clock, SEARCH_DEADLINE);

    let report = deepen(
        &mut Searcher::new(&pipeline, finish()),
        &board,
        &mut allowance,
        8,
    );

    assert_eq!(ITERATION_START_PERCENT, 40);
    assert_eq!(report.completed_depth, 3);
    assert_eq!(clock.reads(), 4);
}

#[test]
fn a_depth_may_start_just_below_the_start_fraction() {
    // 399,999 of 1,000,000 elapsed is below 40%, so depth 2 starts; 40% then stops.
    let pipeline = ValuationPipeline::standard();
    let board = start();
    let clock = ScriptedClock::new(&[0, 399_999, 400_000]);
    let mut allowance = allowance_ending_at(&clock, SEARCH_DEADLINE);

    let report = deepen(
        &mut Searcher::new(&pipeline, finish()),
        &board,
        &mut allowance,
        8,
    );

    assert_eq!(report.completed_depth, 2);
    assert_eq!(clock.reads(), 3);
}

#[test]
fn an_allowance_that_is_already_over_completes_nothing_and_says_so() {
    let pipeline = ValuationPipeline::standard();
    let board = start();
    let clock = ScriptedClock::new(&[SEARCH_DEADLINE + 1]);
    let mut allowance = allowance_ending_at(&clock, SEARCH_DEADLINE);

    let report = deepen(
        &mut Searcher::new(&pipeline, finish()),
        &board,
        &mut allowance,
        8,
    );

    assert_eq!(report, LookaheadReport::nothing_completed(0));
    assert!(!report.has_result());
    assert_eq!(clock.reads(), 1);
}

#[test]
fn a_depth_limit_of_zero_runs_nothing_and_reads_no_clock() {
    let pipeline = ValuationPipeline::standard();
    let clock = ManualClock::at_micros(0);
    let mut allowance = allowance_ending_at(&clock, SEARCH_DEADLINE);

    let report = deepen(
        &mut Searcher::new(&pipeline, finish()),
        &start(),
        &mut allowance,
        0,
    );

    assert_eq!(report, LookaheadReport::nothing_completed(0));
    assert_eq!(clock.reads(), 0);
}

#[test]
fn an_interrupted_iteration_is_discarded_and_the_last_completed_depth_stands() {
    let pipeline = ValuationPipeline::standard();
    let board = start();
    // Five clean reads, then the clock jumps past the deadline: the deepest
    // iterations cannot finish, so one of them is cut off mid-way.
    let clock = ScriptedClock::expiring_after(5);
    let mut allowance = allowance_ending_at(&clock, SEARCH_DEADLINE);
    let mut searcher = Searcher::new(&pipeline, finish());

    let report = deepen(&mut searcher, &board, &mut allowance, 12);

    assert!(report.has_result());
    assert!(report.completed_depth >= 1 && report.completed_depth < 12);
    let expected = fixed(&pipeline, &board, report.completed_depth);
    assert_eq!(report.best, expected.best);
    assert_eq!(report.principal_score, expected.principal_score);

    // Positions of the abandoned iteration still count as explored work.
    let mut replay = Searcher::new(&pipeline, finish());
    let mut completed_work = 0;
    for depth in 1..=report.completed_depth {
        completed_work += replay.search_fixed(&board, depth).nodes_explored;
    }
    assert!(report.nodes_explored > completed_work);
    assert_eq!(report.nodes_explored, searcher.nodes_visited());
}

#[test]
fn identical_input_and_clock_give_identical_reports() {
    let pipeline = ValuationPipeline::standard();
    let board = start();

    let run = || {
        let clock = ScriptedClock::expiring_after(4);
        let mut allowance = allowance_ending_at(&clock, SEARCH_DEADLINE);
        deepen(
            &mut Searcher::new(&pipeline, finish()),
            &board,
            &mut allowance,
            12,
        )
    };

    assert_eq!(run(), run());
}

fn cheap() -> ValuationPipeline<((), tiger_engine::valuation::Weighted<LengthAdvantage>)> {
    ValuationPipeline::empty().with(LengthAdvantage, 250)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(60))]

    #[test]
    fn deepening_reports_the_fixed_depth_answer_at_the_depth_it_reached(
        spec in state_spec(),
        limit in 1u16..=3,
    ) {
        let board = ingest(&realize(&spec)).expect("a duel converts");
        let pipeline = cheap();
        let clock = ManualClock::at_micros(0);
        let mut allowance = allowance_ending_at(&clock, 10_000_000_000);

        let report = deepen(&mut Searcher::new(&pipeline, finish()), &board, &mut allowance, limit);

        // The driver stops at the first depth whose answer is already decided.
        let mut expected_depth = limit;
        for depth in 1..=limit {
            if decided(&fixed(&pipeline, &board, depth)) {
                expected_depth = depth;
                break;
            }
        }
        let expected = fixed(&pipeline, &board, expected_depth);
        prop_assert_eq!(report.completed_depth, expected_depth);
        prop_assert_eq!(report.best, expected.best);
        prop_assert_eq!(report.principal_score, expected.principal_score);
    }
}

// ---- the interruptible search underneath -----------------------------------

/// Stops on the `n`th question, counting every question asked.
struct StopOnCheck {
    stop_at: u64,
    asked: u64,
}

impl StopOnCheck {
    fn new(stop_at: u64) -> Self {
        Self { stop_at, asked: 0 }
    }
}

impl StopSignal for StopOnCheck {
    fn should_stop(&mut self) -> bool {
        self.asked += 1;
        self.asked >= self.stop_at
    }
}

#[test]
fn a_search_asks_the_stop_signal_once_per_visited_node() {
    let pipeline = ValuationPipeline::standard();
    let mut counting = StopOnCheck::new(u64::MAX);
    let mut searcher = Searcher::new(&pipeline, finish());

    let report = searcher
        .search_until(&start(), 3, &mut counting)
        .expect("nothing asked it to stop");

    assert_eq!(counting.asked, report.nodes_explored);
}

#[test]
fn a_search_that_is_never_stopped_matches_the_plain_fixed_depth_search() {
    let pipeline = ValuationPipeline::standard();
    let board = start();

    let report = Searcher::with_order(&pipeline, finish(), NaturalOrder)
        .search_until(&board, 3, &mut NeverStop)
        .expect("never stopped");

    assert_eq!(report, fixed(&pipeline, &board, 3));
}

#[test]
fn a_stopped_search_returns_nothing_and_counts_only_the_nodes_it_visited() {
    let pipeline = ValuationPipeline::standard();
    let mut searcher = Searcher::new(&pipeline, finish());

    let result = searcher.search_until(&start(), 4, &mut StopOnCheck::new(50));

    assert_eq!(result, None);
    assert_eq!(searcher.nodes_visited(), 49);
}

#[test]
fn an_abandoned_search_leaves_the_searcher_fit_for_the_next_one() {
    let pipeline = ValuationPipeline::standard();
    let board = start();
    let mut searcher = Searcher::new(&pipeline, finish());
    searcher.search_fixed(&board, 2);
    assert_eq!(
        searcher.search_until(&board, 4, &mut StopOnCheck::new(300)),
        None
    );

    let resumed = searcher.search_fixed(&board, 4);

    let expected = fixed(&pipeline, &board, 4);
    assert_eq!(resumed.best, expected.best);
    assert_eq!(resumed.principal_score, expected.principal_score);
}
