//! The melee profiling harness (003 T016): search speed and depth on a fixed
//! suite of generated midgame melees under the production allowance. The heavy
//! run is `#[ignore]`d and driven by `scripts/run-profile melee` in release
//! mode; the fast tests pin the suite and the report.

mod support;

use std::time::{Duration, Instant};

use tiger_engine::arena::ingest::ingest_melee;
use tiger_engine::arena::melee::MeleeBoard;
use tiger_engine::gateway::clock::SystemClock;
use tiger_engine::lookahead::allowance::SearchAllowance;
use tiger_engine::lookahead::deepening::{DEPTH_CEILING, deepen_melee};
use tiger_engine::lookahead::paranoid::MeleeSearcher;
use tiger_engine::rules_core::{Clock, RequestTiming};
use tiger_engine::valuation::melee::MeleeValuation;
use tiger_engine::valuation::melee::finish::MeleeFinish;
use tiger_engine::valuation::melee::weights::DEFAULT_MELEE_PROFILE;

use support::env_number;
use support::melee_suite::{
    MELEE_SUITE_SEED, MELEE_SUITE_SIZE, generate_melee_suite, melee_board_to_state,
};

/// The median completed depth the melee search must reach in the production
/// allowance on four-snake midgames (plan.md §Risks: one full extra turn for
/// every snake is depth 2; the gate asks for one more).
const DEPTH_GATE: u16 = 3;

#[derive(Clone, Debug, PartialEq)]
struct SearchFacts {
    snakes: usize,
    depths: Vec<u16>,
    nodes_total: u64,
    nodes_per_second_aggregate: f64,
    slowest_ms: f64,
    decisive_early_stops: u32,
}

fn measure_search(suite: &[MeleeBoard], snakes: usize) -> SearchFacts {
    let valuation = MeleeValuation::standard();
    let clock = SystemClock::new();
    let mut facts = SearchFacts {
        snakes,
        depths: Vec::new(),
        nodes_total: 0,
        nodes_per_second_aggregate: 0.0,
        slowest_ms: 0.0,
        decisive_early_stops: 0,
    };
    let mut elapsed_total = Duration::ZERO;
    for board in suite {
        let started = Instant::now();
        let mut allowance = SearchAllowance::from_request(
            &clock,
            RequestTiming {
                arrived_at: clock.now(),
            },
            Duration::from_millis(500),
        )
        .expect("500 ms is above the reserve");
        let mut searcher = MeleeSearcher::new(&valuation, MeleeFinish::new(&DEFAULT_MELEE_PROFILE));
        let report = deepen_melee(&mut searcher, board, &mut allowance, DEPTH_CEILING);
        let elapsed = started.elapsed();
        if report
            .principal_score
            .is_some_and(|s| searcher.is_decisive(s))
        {
            facts.decisive_early_stops += 1;
        }
        facts.depths.push(report.completed_depth);
        facts.nodes_total += report.nodes_explored;
        elapsed_total += elapsed;
        facts.slowest_ms = facts.slowest_ms.max(elapsed.as_secs_f64() * 1000.0);
    }
    facts.nodes_per_second_aggregate = facts.nodes_total as f64 / elapsed_total.as_secs_f64();
    facts
}

fn median(depths: &[u16]) -> u16 {
    let mut sorted = depths.to_vec();
    sorted.sort_unstable();
    sorted[sorted.len() / 2]
}

fn render(facts: &SearchFacts) -> String {
    let (min, max) = (
        depths_min(&facts.depths),
        facts.depths.iter().copied().max().unwrap_or(0),
    );
    let mean = facts.depths.iter().map(|&d| f64::from(d)).sum::<f64>() / facts.depths.len() as f64;
    format!(
        "melee_snakes: {}\nmelee_suite_size: {}\nsearch_completed_depth_min_median_max: {min} {} {max}     (mean {mean:.2})\nsearch_decisive_early_stops: {}\nsearch_nodes_total: {}\nsearch_nodes_per_second_aggregate: {:.0}\nsearch_slowest_decision_ms: {:.1}\ndepth_gate: median >= {DEPTH_GATE} -> {}\n",
        facts.snakes,
        facts.depths.len(),
        median(&facts.depths),
        facts.decisive_early_stops,
        facts.nodes_total,
        facts.nodes_per_second_aggregate,
        facts.slowest_ms,
        if median(&facts.depths) >= DEPTH_GATE {
            "PASSED"
        } else {
            "NOT PASSED"
        },
    )
}

fn depths_min(depths: &[u16]) -> u16 {
    depths.iter().copied().min().unwrap_or(0)
}

// ---- the suite ------------------------------------------------------------------------------

#[test]
fn the_suite_holds_valid_midgame_melees_of_the_requested_size() {
    for snakes in [3, 4] {
        let suite = generate_melee_suite(MELEE_SUITE_SEED, snakes, MELEE_SUITE_SIZE);

        assert_eq!(suite.len(), MELEE_SUITE_SIZE, "{snakes} snakes");
        for board in &suite {
            assert_eq!(usize::from(board.alive_count()), snakes);
            assert!((20..=45).contains(&board.ply()), "ply {}", board.ply());
            assert!(!board.pellets().is_empty(), "some food left");
            let state = melee_board_to_state(board);
            let back = ingest_melee(&state).expect("the position survives the reference model");
            assert_eq!(back.occupied(), board.occupied());
        }
    }
}

#[test]
fn the_suite_is_reproducible_and_depends_on_its_seed() {
    let a = generate_melee_suite(MELEE_SUITE_SEED, 4, 10);
    let b = generate_melee_suite(MELEE_SUITE_SEED, 4, 10);
    let c = generate_melee_suite(MELEE_SUITE_SEED + 1, 4, 10);

    let occupied =
        |suite: &[MeleeBoard]| suite.iter().map(MeleeBoard::occupied).collect::<Vec<_>>();
    assert_eq!(occupied(&a), occupied(&b));
    assert_ne!(occupied(&a), occupied(&c));
}

#[test]
fn the_report_states_the_gate_from_the_median_depth() {
    let passing = SearchFacts {
        snakes: 4,
        depths: vec![2, 3, 5, 3, 4],
        nodes_total: 1000,
        nodes_per_second_aggregate: 1.0,
        slowest_ms: 1.0,
        decisive_early_stops: 0,
    };
    let failing = SearchFacts {
        depths: vec![1, 2, 2, 3, 4],
        ..passing.clone()
    };

    assert!(render(&passing).contains("median >= 3 -> PASSED"));
    assert!(render(&failing).contains("median >= 3 -> NOT PASSED"));
    assert!(render(&passing).contains("search_completed_depth_min_median_max: 2 3 5"));
}

/// Run by `scripts/run-profile melee` in release mode; prints the report the
/// evidence file quotes. `PROFILE_MELEE_SNAKES` (default 4) picks the suite.
#[test]
#[ignore = "release-mode measurement, run through scripts/run-profile melee"]
fn profile_the_melee_search() {
    let snakes: usize = env_number("PROFILE_MELEE_SNAKES", 4);
    let suite = generate_melee_suite(MELEE_SUITE_SEED, snakes, MELEE_SUITE_SIZE);
    let facts = measure_search(&suite, snakes);

    println!("{}", render(&facts));

    assert!(!facts.depths.contains(&0), "a position completed no depth");
    assert!(
        median(&facts.depths) >= DEPTH_GATE,
        "the depth gate is not met"
    );
}
