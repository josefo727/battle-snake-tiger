mod support;

use std::sync::Arc;

use proptest::prelude::*;
use tiger_engine::arena::ingest::{direction_of, ingest, ingest_melee};
use tiger_engine::lookahead::minimax::Searcher;
use tiger_engine::lookahead::ordering::NaturalOrder;
use tiger_engine::lookahead::paranoid::MeleeSearcher;
use tiger_engine::rules_core::{
    Direction, MonotonicInstant, RequestTiming, Scope, TurnRequestDto, classify,
    decide_unsupported, decide_within_deadline, declared_timeout, direction_to_wire,
    response_deadline, to_turn_state,
};
use tiger_engine::valuation::StandardPipeline;
use tiger_engine::valuation::finish::Finish;
use tiger_engine::valuation::melee::MeleeValuation;
use tiger_engine::valuation::melee::finish::MeleeFinish;
use tiger_engine::valuation::melee::weights::DEFAULT_MELEE_PROFILE;
use tiger_engine::valuation::weights::DEFAULT_PROFILE;
use tiger_engine::verdict::report::{Diagnostic, EnginePath, SelectionReason, VerdictReport};
use tiger_engine::verdict::service::VerdictService;

use support::clock::ManualClock;
use support::{request_from_bodies, request_with};

const ARRIVAL: u64 = 1_000_000;

fn arrival() -> MonotonicInstant {
    MonotonicInstant {
        microseconds: ARRIVAL,
    }
}

fn service(clock: &Arc<ManualClock>, depth_limit: u16) -> VerdictService {
    VerdictService::new(clock.clone()).with_depth_limit(depth_limit)
}

fn facing() -> TurnRequestDto {
    request_from_bodies(
        &[&[(4, 5), (3, 5), (2, 5)], &[(7, 5), (8, 5), (9, 5)]],
        &[90, 90],
        0,
        &[(5, 8), (6, 2)],
    )
}

/// They have one health left, so they starve this turn whatever they do.
fn forced_win() -> TurnRequestDto {
    request_from_bodies(
        &[&[(5, 5), (5, 4), (5, 3)], &[(2, 2), (2, 1), (2, 0)]],
        &[90, 1],
        0,
        &[],
    )
}

/// Cornered: North is the only free cell and a longer snake can meet us there.
fn cornered() -> TurnRequestDto {
    request_from_bodies(
        &[&[(0, 0), (1, 0), (2, 0)], &[(1, 1), (1, 2), (1, 3), (1, 4)]],
        &[90, 90],
        0,
        &[],
    )
}

/// What a plain fixed-depth search says about the request's duel.
fn searched(request: &TurnRequestDto, depth: u16) -> (Direction, i32) {
    let state = to_turn_state(request).expect("a supported request");
    let board = ingest(&state).expect("a duel");
    let pipeline = StandardPipeline::standard();
    let report = Searcher::with_order(&pipeline, Finish::new(&DEFAULT_PROFILE), NaturalOrder)
        .search_fixed(&board, depth);
    (
        direction_of(report.best.expect("a completed depth")),
        report.principal_score.expect("a completed depth"),
    )
}

// ---- the duel path ------------------------------------------------------------

#[test]
fn a_duel_answers_with_the_search_heading_at_the_completed_depth() {
    let clock = Arc::new(ManualClock::at_micros(ARRIVAL));
    let request = facing();

    let report = service(&clock, 3).decide(&request, arrival());

    let (expected_move, expected_score) = searched(&request, 3);
    assert_eq!(report.engine_path, EnginePath::DuelSearch);
    assert_eq!(
        report.selection_reason,
        SelectionReason::SearchCompletedDepth
    );
    assert_eq!(report.search_depth, 3);
    assert!(report.nodes_explored > 0);
    assert_eq!(report.selected_move, expected_move);
    assert_eq!(report.principal_score, Some(expected_score));
    assert!(!report.fallback_used);
    assert_eq!(report.diagnostic, Diagnostic::None);
    assert_eq!(report.elapsed_us, 0, "the manual clock never moved");
}

#[test]
fn a_proven_win_is_reported_as_a_terminal_win_at_the_depth_that_proves_it() {
    let clock = Arc::new(ManualClock::at_micros(ARRIVAL));

    let report = service(&clock, 6).decide(&forced_win(), arrival());

    assert_eq!(report.engine_path, EnginePath::DuelSearch);
    assert_eq!(report.selection_reason, SelectionReason::SearchTerminalWin);
    // A win still stops the deepening at the depth that proves it: nothing
    // deeper can better a forced win.
    assert_eq!(report.search_depth, 1);
    assert_eq!(
        report.principal_score,
        Some(DEFAULT_PROFILE.win_score - DEFAULT_PROFILE.ply_penalty)
    );
}

#[test]
fn a_proven_loss_is_still_a_completed_search_not_a_win() {
    let clock = Arc::new(ManualClock::at_micros(ARRIVAL));

    let report = service(&clock, 6).decide(&cornered(), arrival());

    assert_eq!(report.engine_path, EnginePath::DuelSearch);
    assert_eq!(
        report.selection_reason,
        SelectionReason::SearchCompletedDepth
    );
    // The loss is proven at the first ply and stays proven at every depth after
    // it, but the driver no longer stops there: a verdict against us is only as
    // good as the opponent model behind it, so the budget goes on looking for
    // the line that lasts longest (iteration 20).
    assert_eq!(report.search_depth, 6);
    assert_eq!(
        report.principal_score,
        Some(-(DEFAULT_PROFILE.win_score - DEFAULT_PROFILE.ply_penalty))
    );
    assert!(!report.fallback_used);
}

// ---- when the search cannot answer ---------------------------------------------

#[test]
fn when_no_depth_completes_the_reused_safety_decision_answers_and_says_why() {
    // The search allowance ends 370 ms after arrival and the response deadline
    // 380 ms after: 375 ms leaves the safety engine time, 400 ms does not.
    for (label, clock_at) in [
        ("between the two deadlines", 375_000),
        ("past both", 400_000),
    ] {
        let clock = Arc::new(ManualClock::at_micros(ARRIVAL + clock_at));
        let request = facing();

        let report = service(&clock, 6).decide(&request, arrival());

        let Scope::Supported(state) = classify(&request) else {
            panic!("the request is supported");
        };
        let deadline = response_deadline(
            RequestTiming {
                arrived_at: arrival(),
            },
            declared_timeout(&request),
        )
        .expect("above the reserve");
        let reused = decide_within_deadline(&state, clock.as_ref(), arrival(), deadline)
            .expect("a validated state resolves");
        assert_eq!(report.engine_path, EnginePath::SafetyFallback, "{label}");
        assert_eq!(
            report.selection_reason,
            SelectionReason::BudgetExhaustedBeforeFirstDepth,
            "{label}"
        );
        assert_eq!(report.selected_move, reused.selected_move, "{label}");
        assert_eq!(report.search_depth, 0, "{label}");
        assert_eq!(report.principal_score, None, "{label}");
        assert!(report.fallback_used, "{label}");
        assert_eq!(report.diagnostic, Diagnostic::DeadlineCutoff, "{label}");
        assert_eq!(
            report.nodes_explored,
            u64::try_from(reused.nodes_explored).unwrap(),
            "{label}: the search visited nothing"
        );
        assert_eq!(report.elapsed_us, clock_at, "{label}");
    }
}

// ---- the other routes ----------------------------------------------------------

/// What a plain fixed-depth paranoid search says about the request's melee.
fn melee_searched(request: &TurnRequestDto, depth: u16) -> (Direction, i32) {
    let state = to_turn_state(request).expect("a supported request");
    let board = ingest_melee(&state).expect("a melee");
    let valuation = MeleeValuation::standard();
    let report = MeleeSearcher::with_order(
        &valuation,
        MeleeFinish::new(&DEFAULT_MELEE_PROFILE),
        NaturalOrder,
    )
    .search_fixed(&board, depth);
    (
        direction_of(report.best.expect("a completed depth")),
        report.principal_score.expect("a completed depth"),
    )
}

// ---- the melee path -----------------------------------------------------------

/// A melee we cannot survive: we have one health left, so we starve this turn
/// whatever anyone does.
fn lost_melee() -> TurnRequestDto {
    request_from_bodies(
        &[
            &[(5, 5), (5, 4), (5, 3)],
            &[(2, 2), (2, 1), (2, 0)],
            &[(9, 9), (9, 8), (9, 7)],
        ],
        &[1, 90, 90],
        0,
        &[],
    )
}

#[test]
fn a_melee_the_search_proved_lost_is_handed_to_the_rollouts() {
    // The proof holds only against an opponent model in which all three rivals
    // play the worst line for us every turn. On the ladder on 2026-09-25 that
    // model called a melee lost from turn 105 and the move for the four turns
    // that followed came from the fixed heading order among headings it had
    // scored the same kind of nothing. The rollouts get the last word, and the
    // report says so rather than passing it off as an ordinary search.
    let clock = Arc::new(ManualClock::at_micros(ARRIVAL));

    let report = service(&clock, 4).decide(&lost_melee(), arrival());

    assert_eq!(report.engine_path, EnginePath::MeleeSearch);
    assert_eq!(
        report.selection_reason,
        SelectionReason::SearchLostSoRolloutsChose
    );
    assert!(
        report.principal_score.is_some_and(|score| score < 0),
        "the search still reports what it found"
    );
    assert!(!report.fallback_used, "this is not the safety engine");
    assert_eq!(report.diagnostic, Diagnostic::None);
}

#[test]
fn a_melee_that_is_merely_bad_is_left_to_the_search() {
    // Only a proof is overruled. Anything short of one is the search's to call,
    // or the rollouts would be deciding the whole game.
    let clock = Arc::new(ManualClock::at_micros(ARRIVAL));
    let request = request_with("v1.2.3", 4, 500, 1, &[(5, 6), (2, 3)]);

    let report = service(&clock, 2).decide(&request, arrival());

    assert_eq!(
        report.selection_reason,
        SelectionReason::SearchCompletedDepth
    );
}

#[test]
fn a_melee_answers_with_the_paranoid_heading_at_the_completed_depth() {
    let clock = Arc::new(ManualClock::at_micros(ARRIVAL));
    for snakes in [3, 4] {
        let request = request_with("v1.2.3", snakes, 500, 1, &[(5, 6), (2, 3)]);

        let report = service(&clock, 2).decide(&request, arrival());

        let (direction, score) = melee_searched(&request, 2);
        assert_eq!(report.engine_path, EnginePath::MeleeSearch, "{snakes}");
        assert_eq!(report.selected_move, direction, "{snakes}");
        assert_eq!(report.principal_score, Some(score), "{snakes}");
        assert_eq!(report.search_depth, 2, "{snakes}");
        assert!(report.nodes_explored > 0, "{snakes}");
        assert!(!report.fallback_used, "{snakes}");
        assert_eq!(
            report.selection_reason,
            SelectionReason::SearchCompletedDepth
        );
        assert_eq!(report.diagnostic, Diagnostic::None);
    }
}

#[test]
fn a_melee_with_no_search_time_uses_the_safety_engine_and_says_so() {
    // 125 ms declared: the search deadline is already in the past on arrival.
    let clock = Arc::new(ManualClock::at_micros(ARRIVAL));
    let request = request_with("v1.2.3", 4, 125, 0, &[]);

    let report = service(&clock, 6).decide(&request, arrival());

    assert_eq!(report.engine_path, EnginePath::SafetyFallback);
    assert_eq!(
        report.selection_reason,
        SelectionReason::BudgetExhaustedBeforeFirstDepth
    );
    assert!(report.fallback_used);
    assert_eq!(report.search_depth, 0);
    assert_eq!(report.diagnostic, Diagnostic::DeadlineCutoff);
    assert_wire_direction(&report);
}

#[test]
fn an_unsupported_request_gets_exactly_what_the_reused_best_effort_returns() {
    let clock = Arc::new(ManualClock::at_micros(ARRIVAL));
    let request = request_with("v9.9.9", 2, 500, 0, &[]);

    let report = service(&clock, 6).decide(&request, arrival());

    let Scope::Unsupported(context) = classify(&request) else {
        panic!("the request is unsupported");
    };
    let reused = decide_unsupported(&context, clock.as_ref(), arrival());
    assert_eq!(report.engine_path, EnginePath::UnsupportedFallback);
    assert_eq!(
        report.selection_reason,
        SelectionReason::UnsupportedBestEffort
    );
    assert_eq!(report.selected_move, reused.selected_move);
    assert!(report.fallback_used);
    assert_eq!(report.diagnostic, Diagnostic::UnsupportedScope);
    assert_eq!(report.search_depth, 0);
    assert_eq!(report.nodes_explored, 0);
}

#[test]
fn the_same_request_and_clock_give_the_same_report() {
    let run = || {
        let clock = Arc::new(ManualClock::at_micros(ARRIVAL));
        service(&clock, 3).decide(&facing(), arrival())
    };

    assert_eq!(run(), run());
}

#[test]
fn a_timeout_barely_above_the_reserve_leaves_no_search_time_and_uses_the_safety_engine() {
    // 125 ms declared: response deadline 5 ms after arrival, search deadline
    // (10 ms earlier) already in the past.
    let clock = Arc::new(ManualClock::at_micros(ARRIVAL));
    let request = request_with("v1.2.3", 2, 125, 0, &[]);

    let report = service(&clock, 6).decide(&request, arrival());

    assert_eq!(report.engine_path, EnginePath::SafetyFallback);
    assert_eq!(
        report.selection_reason,
        SelectionReason::BudgetExhaustedBeforeFirstDepth
    );
    assert!(report.fallback_used);
    assert_eq!(report.diagnostic, Diagnostic::DeadlineCutoff);
}

fn assert_wire_direction(report: &VerdictReport) {
    assert!(["up", "right", "down", "left"].contains(&direction_to_wire(report.selected_move)));
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(60))]

    #[test]
    fn every_request_shape_gets_a_platform_move_from_the_routed_engine(
        version in prop_oneof![Just("v1.2.3"), Just("cli"), Just("v9.9.9")],
        snakes in 0usize..=4,
        timeout in 100i64..=600,
        you in 0usize..=1,
    ) {
        let you = you.min(snakes.saturating_sub(1));
        let clock = Arc::new(ManualClock::at_micros(ARRIVAL));
        let request = request_with(version, snakes, timeout, you, &[]);

        let report = service(&clock, 2).decide(&request, arrival());

        assert_wire_direction(&report);
        let certified = version != "v9.9.9" && timeout > 120 && snakes >= 2;
        // The search stops 10 ms before the response deadline (120 ms before the
        // declared timeout), so a duel needs more than 130 ms to search at all.
        let searchable = timeout > 130;
        let expected_path = if !certified {
            EnginePath::UnsupportedFallback
        } else if snakes == 2 && searchable {
            EnginePath::DuelSearch
        } else if snakes >= 3 && searchable {
            EnginePath::MeleeSearch
        } else {
            EnginePath::SafetyFallback
        };
        prop_assert_eq!(report.engine_path, expected_path);
        if matches!(report.engine_path, EnginePath::DuelSearch | EnginePath::MeleeSearch) {
            prop_assert!((1..=2).contains(&report.search_depth));
            prop_assert!(!report.fallback_used);
            prop_assert!(report.principal_score.is_some());
        } else {
            prop_assert_eq!(report.search_depth, 0);
            prop_assert_eq!(report.principal_score, None);
        }
    }
}

#[test]
fn a_melee_decided_over_two_threads_is_the_same_decision() {
    let clock = Arc::new(ManualClock::at_micros(ARRIVAL));
    let request = request_with("v1.2.3", 4, 500, 1, &[(5, 6), (2, 3)]);

    let one = service(&clock, 3).decide(&request, arrival());
    let two = service(&clock, 3)
        .with_threads(2)
        .decide(&request, arrival());

    assert_eq!(two.engine_path, EnginePath::MeleeSearch);
    assert_eq!(two.selected_move, one.selected_move);
    assert_eq!(two.principal_score, one.principal_score);
    assert_eq!(two.search_depth, 3);
}
