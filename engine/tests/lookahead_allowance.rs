mod support;

use core::time::Duration;

use tiger_engine::arena::heading::Heading;
use tiger_engine::lookahead::allowance::{
    POLL_INTERVAL_NODES, SEARCH_TAIL_MARGIN, SearchAllowance,
};
use tiger_engine::lookahead::ledger::LookaheadReport;
use tiger_engine::rules_core::{MonotonicInstant, RequestTiming};

use support::clock::ManualClock;

const ARRIVAL: u64 = 1_000_000;

fn timing() -> RequestTiming {
    RequestTiming {
        arrived_at: MonotonicInstant {
            microseconds: ARRIVAL,
        },
    }
}

/// The search deadline for a 500 ms request: arrival + (500 - 120 - 10) ms.
const DEADLINE: u64 = ARRIVAL + 370_000;

fn allowance(clock: &ManualClock) -> SearchAllowance<'_> {
    SearchAllowance::from_request(clock, timing(), Duration::from_millis(500))
        .expect("a 500 ms timeout exceeds the response reserve")
}

#[test]
fn the_deadline_is_the_response_deadline_minus_the_tail_margin() {
    let clock = ManualClock::at_micros(ARRIVAL);

    let a = allowance(&clock);

    assert_eq!(SEARCH_TAIL_MARGIN, Duration::from_millis(10));
    assert_eq!(a.deadline().microseconds, DEADLINE);
    assert_eq!(
        clock.reads(),
        0,
        "building an allowance must not read the clock"
    );
}

#[test]
fn a_timeout_at_or_below_the_response_reserve_has_no_allowance() {
    let clock = ManualClock::at_micros(ARRIVAL);

    assert!(SearchAllowance::from_request(&clock, timing(), Duration::from_millis(120)).is_none());
    assert!(SearchAllowance::from_request(&clock, timing(), Duration::from_millis(50)).is_none());
}

#[test]
fn polling_on_demand_stops_exactly_at_the_deadline() {
    let clock = ManualClock::at_micros(DEADLINE - 1);
    let mut a = allowance(&clock);

    assert!(!a.poll_now(), "one microsecond before the deadline");

    clock.set_micros(DEADLINE);
    assert!(a.poll_now(), "at the deadline");

    clock.set_micros(DEADLINE + 5_000);
    assert!(a.poll_now(), "after the deadline");
}

#[test]
fn the_clock_is_read_only_every_poll_interval_of_visited_nodes() {
    let clock = ManualClock::at_micros(ARRIVAL);
    let mut a = allowance(&clock);

    for _ in 0..POLL_INTERVAL_NODES - 1 {
        assert!(!a.should_stop());
    }
    assert_eq!(clock.reads(), 0, "no read before the interval elapses");

    assert!(!a.should_stop());
    assert_eq!(clock.reads(), 1, "one read on the interval's last node");

    for _ in 0..POLL_INTERVAL_NODES {
        a.should_stop();
    }
    assert_eq!(clock.reads(), 2);
}

#[test]
fn an_expired_deadline_is_only_noticed_at_the_next_poll_point() {
    let clock = ManualClock::at_micros(ARRIVAL);
    let mut a = allowance(&clock);
    for _ in 0..500 {
        assert!(!a.should_stop());
    }

    clock.set_micros(DEADLINE + 1);

    for node in 501..POLL_INTERVAL_NODES {
        assert!(!a.should_stop(), "node {node} is between polls");
    }
    assert!(a.should_stop(), "the poll point sees the expired deadline");
}

#[test]
fn once_the_deadline_is_seen_the_answer_stays_stop_without_reading_the_clock() {
    let clock = ManualClock::at_micros(DEADLINE);
    let mut a = allowance(&clock);
    assert!(a.poll_now());
    let reads_at_expiry = clock.reads();

    clock.set_micros(ARRIVAL);
    assert!(a.should_stop());
    assert!(a.poll_now());

    assert_eq!(clock.reads(), reads_at_expiry);
}

#[test]
fn polling_on_demand_restarts_the_node_count() {
    let clock = ManualClock::at_micros(ARRIVAL);
    let mut a = allowance(&clock);
    for _ in 0..1_000 {
        a.should_stop();
    }

    a.poll_now();
    let after_demand = clock.reads();
    for _ in 0..POLL_INTERVAL_NODES - 1 {
        a.should_stop();
    }

    assert_eq!(after_demand, 1);
    assert_eq!(
        clock.reads(),
        1,
        "the interval counts from the on-demand poll"
    );
    a.should_stop();
    assert_eq!(clock.reads(), 2);
}

#[test]
fn a_report_records_depth_nodes_best_heading_and_principal_score() {
    let done = LookaheadReport::completed(7, 123_456, Heading::East, -42);
    let none = LookaheadReport::nothing_completed(900);

    assert_eq!(done.completed_depth, 7);
    assert_eq!(done.nodes_explored, 123_456);
    assert_eq!(done.best, Some(Heading::East));
    assert_eq!(done.principal_score, Some(-42));
    assert!(done.has_result());
    assert_eq!(none.completed_depth, 0);
    assert_eq!(none.nodes_explored, 900);
    assert_eq!(none.best, None);
    assert_eq!(none.principal_score, None);
    assert!(!none.has_result());
}

#[test]
fn time_left_reports_the_remaining_allowance_from_one_clock_read() {
    let clock = ManualClock::at_micros(ARRIVAL + 100_000);
    let mut a = allowance(&clock);

    assert_eq!(a.time_left(), Duration::from_micros(270_000));
    assert_eq!(clock.reads(), 1);
}

#[test]
fn time_left_is_zero_from_the_deadline_on_and_stays_zero_without_reading() {
    let clock = ManualClock::at_micros(DEADLINE);
    let mut a = allowance(&clock);

    assert_eq!(a.time_left(), Duration::ZERO);
    clock.set_micros(ARRIVAL);

    assert_eq!(a.time_left(), Duration::ZERO, "expiry is sticky");
    assert_eq!(clock.reads(), 1);
    assert!(a.should_stop());
}

#[test]
fn time_left_restarts_the_node_count() {
    let clock = ManualClock::at_micros(ARRIVAL);
    let mut a = allowance(&clock);
    for _ in 0..POLL_INTERVAL_NODES - 1 {
        a.should_stop();
    }

    a.time_left();
    for _ in 0..POLL_INTERVAL_NODES - 1 {
        a.should_stop();
    }

    assert_eq!(clock.reads(), 1);
    a.should_stop();
    assert_eq!(clock.reads(), 2);
}
