//! The profiling harness (T030): kernel throughput against the reused resolver,
//! search speed and depth on a fixed suite of generated midgame duels, and the
//! rate at which a search revisits positions. The heavy run is `#[ignore]`d and
//! driven by `scripts/run-profile` in release mode; the fast tests here pin the
//! harness's own pieces so its numbers can be trusted.

mod support;

use std::collections::HashMap;
use std::hint::black_box;
use std::time::Instant;

use tiger_engine::arena::cellset::Cell;
use tiger_engine::arena::duel::{Advance, DuelBoard, Side};
use tiger_engine::arena::heading::Heading;
use tiger_engine::arena::ingest::direction_of;
use tiger_engine::lookahead::ordering::{HeadingOrder, LearnedOrder};
use tiger_engine::rules_core::{JointMoves, TurnState, resolve_turn};
use tiger_engine::valuation::StandardPipeline;
use tiger_engine::valuation::finish::Finish;
use tiger_engine::valuation::weights::DEFAULT_PROFILE;

use support::env_number;
use support::suite::{SUITE_SEED, SUITE_SIZE, board_to_state, generate_suite};

/// Share of visited positions, in percent, at which a transposition table is
/// worth building (ADR 0005, plan.md evidence gate).
const REPEAT_GATE_PERCENT: u64 = 15;

type Key = Vec<u8>;

// ---- repeat counting ------------------------------------------------------------------

/// A position's identity for repeat counting: both bodies head first, both
/// health values and the pellets, but not the ply counter.
fn position_key(board: &DuelBoard) -> Key {
    let mut key = Vec::with_capacity(48);
    for side in [Side::Us, Side::Them] {
        let serpent = board.serpent(side);
        key.push(serpent.length());
        key.push(serpent.vigor());
        key.extend(serpent.body().map(Cell::index));
    }
    key.extend_from_slice(&board.pellets().bits().to_le_bytes());
    key
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct RepeatStats {
    nodes: u64,
    /// Visits to a position already seen in this search.
    raw_repeats: u64,
    /// Repeats a depth-preferred table could answer: the stored depth is at least
    /// the remaining depth of the visit.
    usable_repeats: u64,
}

#[derive(Default)]
struct RepeatCounter {
    seen: HashMap<Key, u16>,
    stats: RepeatStats,
}

impl RepeatCounter {
    /// A position was reached with `remaining` plies still to search below it.
    fn visit(&mut self, key: Key, remaining: u16) {
        self.stats.nodes += 1;
        match self.seen.get_mut(&key) {
            Some(stored) => {
                self.stats.raw_repeats += 1;
                if *stored >= remaining {
                    self.stats.usable_repeats += 1;
                } else {
                    *stored = remaining;
                }
            }
            None => {
                self.seen.insert(key, remaining);
            }
        }
    }

    /// A game-ending outcome: a visited node with no position to remember.
    fn visit_terminal(&mut self) {
        self.stats.nodes += 1;
    }
}

/// The ADR 0005 gate: at least 15% of visited positions are repeats a table
/// could answer. No sample is no evidence.
fn gate_passes(stats: &RepeatStats) -> bool {
    stats.nodes > 0 && stats.usable_repeats * 100 >= stats.nodes * REPEAT_GATE_PERCENT
}

// ---- throughput -------------------------------------------------------------------------

const JOINT_MOVES: [(Heading, Heading); 16] = {
    let mut all = [(Heading::North, Heading::North); 16];
    let mut i = 0;
    while i < 16 {
        all[i] = (Heading::ALL[i / 4], Heading::ALL[i % 4]);
        i += 1;
    }
    all
};

#[derive(Debug, PartialEq)]
struct Throughput {
    joint_moves: u64,
    kernel_ns_per_advance: f64,
    reference_ns_per_resolve: f64,
}

/// Times every joint move of every position, `rounds` times over, on the kernel
/// and on the reused resolver (states and moves prepared outside the timing).
fn measure_throughput(suite: &[DuelBoard], rounds: u32) -> Throughput {
    let states: Vec<TurnState> = suite.iter().map(board_to_state).collect();
    let moves: Vec<JointMoves> = JOINT_MOVES
        .iter()
        .map(|&(us, them)| {
            JointMoves::try_new(vec![direction_of(us), direction_of(them)], 2).expect("two moves")
        })
        .collect();
    let joint_moves = suite.len() as u64 * 16 * u64::from(rounds);

    let started = Instant::now();
    for _ in 0..rounds {
        for board in suite {
            for &(us, them) in &JOINT_MOVES {
                black_box(board.advance(black_box(us), black_box(them)));
            }
        }
    }
    let kernel = started.elapsed();

    let started = Instant::now();
    for _ in 0..rounds {
        for state in &states {
            for joint in &moves {
                black_box(resolve_turn(black_box(state), black_box(joint)).expect("resolves"));
            }
        }
    }
    let reference = started.elapsed();

    Throughput {
        joint_moves,
        kernel_ns_per_advance: kernel.as_nanos() as f64 / joint_moves as f64,
        reference_ns_per_resolve: reference.as_nanos() as f64 / joint_moves as f64,
    }
}

#[derive(Debug, PartialEq)]
struct DepthSummary {
    min: u16,
    median: u16,
    max: u16,
}

fn summarize_depths(depths: &[u16]) -> DepthSummary {
    let mut sorted = depths.to_vec();
    sorted.sort_unstable();
    DepthSummary {
        min: sorted[0],
        median: sorted[sorted.len() / 2],
        max: sorted[sorted.len() - 1],
    }
}

// ---- the traced search ----------------------------------------------------------------------

/// The search of `Searcher` (learned move order, fixed depth, the standard
/// pipeline) re-implemented with a visit counter, so the harness can see every
/// position without touching production code. A fast test holds it to the real
/// search's node count, value and heading.
struct Tracer<'p> {
    pipeline: &'p StandardPipeline,
    finish: Finish,
    order: LearnedOrder,
    nodes: u64,
    iteration: RepeatCounter,
    run: RepeatCounter,
}

impl Tracer<'_> {
    fn visit(&mut self, board: &DuelBoard, remaining: u16) {
        let key = position_key(board);
        self.run.visit(key.clone(), remaining);
        self.iteration.visit(key, remaining);
    }

    fn visit_terminal(&mut self) {
        self.run.visit_terminal();
        self.iteration.visit_terminal();
    }

    fn search(&mut self, board: &DuelBoard, depth: u16) -> (Heading, i32) {
        let sentinel = self.finish.sentinel();
        self.iteration = RepeatCounter::default();
        let before = self.nodes;
        let (best, score) = self.best_heading(board, depth, 0, -sentinel, sentinel);
        self.order.note_root_best(best);
        self.nodes -= before; // report this iteration's nodes only
        (best, score)
    }

    fn best_heading(
        &mut self,
        board: &DuelBoard,
        depth: u16,
        ply: u16,
        mut alpha: i32,
        beta: i32,
    ) -> (Heading, i32) {
        let mut best = (Heading::ALL[0], -self.finish.sentinel());
        for ours in self.order.arrange(Side::Us, ply) {
            let wins_ties = ply == 0 && ours.index() < best.0.index();
            let floor = if wins_ties { alpha - 1 } else { alpha };
            let score = self.minimizer(board, ours, depth, ply, floor, beta);
            if score > best.1 || (wins_ties && score == best.1) {
                best = (ours, score);
                alpha = alpha.max(score);
            }
            if alpha >= beta {
                self.order.note_cutoff(Side::Us, ply, ours, depth);
                break;
            }
        }
        best
    }

    fn minimizer(
        &mut self,
        board: &DuelBoard,
        ours: Heading,
        depth: u16,
        ply: u16,
        alpha: i32,
        mut beta: i32,
    ) -> i32 {
        let mut best = self.finish.sentinel();
        for theirs in self.order.arrange(Side::Them, ply) {
            self.nodes += 1;
            let score = match board.advance(ours, theirs) {
                Advance::Over(verdict) => {
                    self.visit_terminal();
                    self.finish.score(verdict, ply + 1)
                }
                Advance::Continues(next) => {
                    self.visit(&next, depth - 1);
                    if depth == 1 {
                        self.pipeline.score(&next)
                    } else {
                        self.best_heading(&next, depth - 1, ply + 1, alpha, beta).1
                    }
                }
            };
            best = best.min(score);
            beta = beta.min(best);
            if best <= alpha {
                self.order.note_cutoff(Side::Them, ply, theirs, depth);
                break;
            }
        }
        best
    }
}

#[derive(Debug, PartialEq)]
struct TracedResult {
    best: Heading,
    score: i32,
    /// Nodes of the final iteration alone.
    nodes: u64,
    /// Repeats within the final iteration: one fixed-depth search.
    stats: RepeatStats,
    /// Repeats across the whole deepening run, earlier iterations included.
    cumulative: RepeatStats,
}

/// Deepens through depths `1..=warm_start_depths` and then searches `depth`,
/// reporting the final iteration.
fn traced_search(board: &DuelBoard, depth: u16, warm_start_depths: u16) -> TracedResult {
    let pipeline = StandardPipeline::standard();
    let mut tracer = Tracer {
        pipeline: &pipeline,
        finish: Finish::new(&DEFAULT_PROFILE),
        order: LearnedOrder::new(),
        nodes: 0,
        iteration: RepeatCounter::default(),
        run: RepeatCounter::default(),
    };
    for warm in 1..=warm_start_depths {
        tracer.search(board, warm);
    }
    let (best, score) = tracer.search(board, depth);
    TracedResult {
        best,
        score,
        nodes: tracer.nodes,
        stats: tracer.iteration.stats,
        cumulative: tracer.run.stats,
    }
}

// ---- tests: the suite ---------------------------------------------------------------

#[test]
fn the_suite_holds_two_hundred_valid_midgame_duels() {
    let suite = generate_suite(SUITE_SEED);

    assert_eq!(suite.len(), SUITE_SIZE);
    for board in &suite {
        let ply = board.ply();
        assert!((20..=45).contains(&ply), "ply {ply}");
        for side in [
            tiger_engine::arena::duel::Side::Us,
            tiger_engine::arena::duel::Side::Them,
        ] {
            let serpent = board.serpent(side);
            assert!(serpent.length() >= 3 && serpent.vigor() >= 1);
        }
    }
}

#[test]
fn the_suite_is_reproducible_and_depends_on_its_seed() {
    let keys = |seed| {
        generate_suite(seed)
            .iter()
            .map(position_key)
            .collect::<Vec<_>>()
    };

    let first = keys(SUITE_SEED);

    assert!(!first.is_empty());
    assert_eq!(first, keys(SUITE_SEED));
    assert_ne!(first, keys(SUITE_SEED + 1));
}

#[test]
fn the_suite_is_a_real_midgame_with_growth_and_food_left() {
    let suite = generate_suite(SUITE_SEED);

    assert!(!suite.is_empty());
    let grown = suite
        .iter()
        .filter(|b| {
            [
                tiger_engine::arena::duel::Side::Us,
                tiger_engine::arena::duel::Side::Them,
            ]
            .iter()
            .any(|&s| b.serpent(s).length() > 3)
        })
        .count();
    let with_food = suite.iter().filter(|b| !b.pellets().is_empty()).count();
    assert!(
        grown * 4 >= suite.len(),
        "only {grown} of {} positions show growth",
        suite.len()
    );
    assert!(
        with_food * 2 >= suite.len(),
        "only {with_food} positions still have food"
    );
}

#[test]
fn every_position_survives_a_round_trip_through_the_reference_state() {
    let suite = generate_suite(SUITE_SEED);

    assert!(!suite.is_empty());
    for board in &suite {
        let state = board_to_state(board);
        let again = tiger_engine::arena::ingest::ingest(&state).expect("a duel");

        assert_eq!(position_key(&again), position_key(board));
    }
}

// ---- the position key and the repeat counter ------------------------------------------------

#[test]
fn the_position_key_ignores_the_ply_and_tells_boards_apart() {
    let suite = generate_suite(SUITE_SEED);
    assert!(suite.len() > 2);

    assert_ne!(position_key(&suite[0]), position_key(&suite[1]));
    assert_eq!(position_key(&suite[0]), position_key(&suite[0]));
    assert!(!position_key(&suite[0]).is_empty());
}

#[test]
fn a_repeat_is_usable_only_when_the_stored_depth_covers_the_visit() {
    let mut counter = RepeatCounter::default();
    let (a, b) = (vec![1u8], vec![2u8]);

    counter.visit(a.clone(), 2); // new
    counter.visit(b.clone(), 2); // new
    counter.visit(a.clone(), 1); // seen at depth 2 >= 1: usable
    counter.visit(a.clone(), 3); // seen, but only to depth 2 < 3: repeat, not usable
    counter.visit(a, 3); // stored depth is now 3: usable

    assert_eq!(
        counter.stats,
        RepeatStats {
            nodes: 5,
            raw_repeats: 3,
            usable_repeats: 2
        }
    );
}

#[test]
fn the_transposition_gate_needs_at_least_fifteen_percent_usable_repeats() {
    let stats = |nodes, usable| RepeatStats {
        nodes,
        raw_repeats: usable,
        usable_repeats: usable,
    };

    assert!(gate_passes(&stats(1000, 150)));
    assert!(gate_passes(&stats(1000, 400)));
    assert!(!gate_passes(&stats(1000, 149)));
    assert!(!gate_passes(&stats(0, 0)), "no sample is no evidence");
}

// ---- the instruments ------------------------------------------------------------------------

#[test]
fn the_traced_search_visits_exactly_the_positions_of_the_real_search() {
    use tiger_engine::lookahead::minimax::Searcher;
    use tiger_engine::valuation::StandardPipeline;
    use tiger_engine::valuation::finish::Finish;
    use tiger_engine::valuation::weights::DEFAULT_PROFILE;

    let suite = generate_suite(SUITE_SEED);
    assert!(!suite.is_empty());
    let pipeline = StandardPipeline::standard();

    for board in suite.iter().step_by(20) {
        let mut real = Searcher::new(&pipeline, Finish::new(&DEFAULT_PROFILE));
        for depth in 1..=3 {
            real.search_fixed(board, depth);
        }
        let last = real.search_fixed(board, 4);

        // The tracer warms up on depths 1..=3 exactly as the real searcher did.
        let traced = traced_search(board, 4, 3);

        assert_eq!(traced.nodes, last.nodes_explored);
        assert_eq!(Some(traced.best), last.best);
        assert_eq!(Some(traced.score), last.principal_score);
        assert_eq!(traced.stats.nodes, traced.nodes);
    }
}

#[test]
fn throughput_times_every_joint_move_of_every_position_on_both_resolvers() {
    let suite = generate_suite(SUITE_SEED);
    assert!(suite.len() >= 4);

    let result = measure_throughput(&suite[..4], 2);

    assert_eq!(result.joint_moves, 4 * 16 * 2);
    assert!(result.kernel_ns_per_advance > 0.0);
    assert!(result.reference_ns_per_resolve > 0.0);
}

#[test]
fn depth_summaries_report_min_median_and_max() {
    assert_eq!(
        summarize_depths(&[9, 3, 5, 5]),
        DepthSummary {
            min: 3,
            median: 5,
            max: 9
        }
    );
    assert_eq!(
        summarize_depths(&[4]),
        DepthSummary {
            min: 4,
            median: 4,
            max: 4
        }
    );
    assert_eq!(
        summarize_depths(&[1, 2, 3, 4]),
        DepthSummary {
            min: 1,
            median: 3,
            max: 4
        }
    );
}

// ---- measuring --------------------------------------------------------------------------------

/// Deepest iteration the repeat measurement replays for one position, and every
/// how-many-th position it replays; both can be overridden from the environment
/// for a sensitivity check (`PROFILE_REPEAT_DEPTH_CAP`, `PROFILE_REPEAT_STRIDE`).
const DEFAULT_REPEAT_DEPTH_CAP: u16 = 9;
const DEFAULT_REPEAT_STRIDE: usize = 1;
const THROUGHPUT_ROUNDS: u32 = 200;

fn percent(part: u64, whole: u64) -> f64 {
    if whole == 0 {
        0.0
    } else {
        part as f64 * 100.0 / whole as f64
    }
}

fn median_f64(values: &[f64]) -> f64 {
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).expect("finite"));
    sorted[sorted.len() / 2]
}

struct SuiteFacts {
    size: usize,
    ply_min: u16,
    ply_max: u16,
    mean_snake_length: f64,
    with_growth: usize,
    mean_pellets_left: f64,
}

struct SearchFacts {
    depths: Vec<u16>,
    decisive_early_stops: usize,
    nodes_total: u64,
    nodes_per_second_aggregate: f64,
    nodes_per_second_median: f64,
    slowest_ms: f64,
}

struct RepeatFacts {
    depth_cap: u16,
    final_iteration: RepeatStats,
    whole_run: RepeatStats,
    usable_percent_per_position: Vec<f64>,
}

/// Everything the heavy run learned; deciding what it means is `problems` and
/// `render`, not the measuring.
struct Profile {
    suite: SuiteFacts,
    throughput: Throughput,
    search: SearchFacts,
    repeats: RepeatFacts,
}

fn suite_facts(suite: &[DuelBoard]) -> SuiteFacts {
    let plies: Vec<u16> = suite.iter().map(DuelBoard::ply).collect();
    let lengths: Vec<u32> = suite
        .iter()
        .flat_map(|b| [Side::Us, Side::Them].map(|s| u32::from(b.serpent(s).length())))
        .collect();
    let pellets: u32 = suite.iter().map(|b| b.pellets().len()).sum();
    SuiteFacts {
        size: suite.len(),
        ply_min: *plies.iter().min().expect("a suite"),
        ply_max: *plies.iter().max().expect("a suite"),
        mean_snake_length: f64::from(lengths.iter().sum::<u32>()) / lengths.len() as f64,
        with_growth: suite
            .iter()
            .filter(|b| {
                [Side::Us, Side::Them]
                    .iter()
                    .any(|&s| b.serpent(s).length() > 3)
            })
            .count(),
        mean_pellets_left: f64::from(pellets) / suite.len() as f64,
    }
}

/// Every position searched once with the production settings.
fn measure_search(suite: &[DuelBoard]) -> SearchFacts {
    use std::time::Duration;

    use tiger_engine::gateway::clock::SystemClock;
    use tiger_engine::lookahead::allowance::SearchAllowance;
    use tiger_engine::lookahead::deepening::{DEPTH_CEILING, deepen};
    use tiger_engine::lookahead::minimax::Searcher;
    use tiger_engine::rules_core::{Clock, RequestTiming};

    let pipeline = StandardPipeline::standard();
    let clock = SystemClock::new();
    let mut facts = SearchFacts {
        depths: Vec::new(),
        decisive_early_stops: 0,
        nodes_total: 0,
        nodes_per_second_aggregate: 0.0,
        nodes_per_second_median: 0.0,
        slowest_ms: 0.0,
    };
    let (mut elapsed_total, mut rates) = (Duration::ZERO, Vec::new());
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
        let mut searcher = Searcher::new(&pipeline, Finish::new(&DEFAULT_PROFILE));
        let report = deepen(&mut searcher, board, &mut allowance, DEPTH_CEILING);
        let elapsed = started.elapsed();
        if report
            .principal_score
            .is_some_and(|s| searcher.is_decisive(s))
        {
            facts.decisive_early_stops += 1;
        }
        // A position with no result is recorded as depth 0 and flagged by `problems`.
        facts.depths.push(report.completed_depth);
        facts.nodes_total += report.nodes_explored;
        elapsed_total += elapsed;
        facts.slowest_ms = facts.slowest_ms.max(elapsed.as_secs_f64() * 1000.0);
        rates.push(report.nodes_explored as f64 / elapsed.as_secs_f64());
    }
    facts.nodes_per_second_aggregate = facts.nodes_total as f64 / elapsed_total.as_secs_f64();
    facts.nodes_per_second_median = median_f64(&rates);
    facts
}

/// Repeats within one fixed-depth search, at the depth each position reached.
fn measure_repeats(suite: &[DuelBoard], reached: &[u16]) -> RepeatFacts {
    let depth_cap: u16 = env_number("PROFILE_REPEAT_DEPTH_CAP", DEFAULT_REPEAT_DEPTH_CAP);
    let stride: usize = env_number("PROFILE_REPEAT_STRIDE", DEFAULT_REPEAT_STRIDE);
    let mut facts = RepeatFacts {
        depth_cap,
        final_iteration: RepeatStats::default(),
        whole_run: RepeatStats::default(),
        usable_percent_per_position: Vec::new(),
    };
    for (board, &depth) in suite.iter().zip(reached).step_by(stride) {
        let target = depth.clamp(1, depth_cap);
        let traced = traced_search(board, target, target - 1);
        for (total, part) in [
            (&mut facts.final_iteration, traced.stats),
            (&mut facts.whole_run, traced.cumulative),
        ] {
            total.nodes += part.nodes;
            total.raw_repeats += part.raw_repeats;
            total.usable_repeats += part.usable_repeats;
        }
        facts
            .usable_percent_per_position
            .push(percent(traced.stats.usable_repeats, traced.stats.nodes));
    }
    facts
}

fn measure_profile() -> Profile {
    let suite = generate_suite(SUITE_SEED);
    let throughput = measure_throughput(&suite, THROUGHPUT_ROUNDS);
    let search = measure_search(&suite);
    let repeats = measure_repeats(&suite, &search.depths);
    Profile {
        suite: suite_facts(&suite),
        throughput,
        search,
        repeats,
    }
}

// ---- judging and reporting (no measuring below this line) -----------------------------------------

/// What is wrong with a profile that makes its numbers untrustworthy; empty when
/// nothing is. The transposition gate is a finding, not a problem.
fn problems(profile: &Profile) -> Vec<String> {
    let mut found = Vec::new();
    if profile.suite.size != SUITE_SIZE {
        found.push(format!(
            "the suite has {} positions, not {SUITE_SIZE}",
            profile.suite.size
        ));
    }
    if profile.search.depths.contains(&0) {
        found.push("a position completed no depth at all".to_owned());
    }
    if profile.throughput.kernel_ns_per_advance <= 0.0
        || profile.throughput.reference_ns_per_resolve <= 0.0
    {
        found.push("a throughput timing is not positive".to_owned());
    }
    if profile.repeats.final_iteration.nodes == 0 {
        found.push("the repeat measurement visited no positions".to_owned());
    }
    found
}

fn render(profile: &Profile) -> String {
    let (suite, throughput, search, repeats) = (
        &profile.suite,
        &profile.throughput,
        &profile.search,
        &profile.repeats,
    );
    let depth = summarize_depths(&search.depths);
    let mean_depth =
        search.depths.iter().map(|&d| f64::from(d)).sum::<f64>() / search.depths.len() as f64;
    let at_gate = repeats
        .usable_percent_per_position
        .iter()
        .filter(|&&p| p >= REPEAT_GATE_PERCENT as f64)
        .count();
    let (last, run) = (&repeats.final_iteration, &repeats.whole_run);
    let lines = [
        "=== profile report ===".to_owned(),
        format!("suite_size: {}", suite.size),
        format!("suite_seed: {SUITE_SEED:#x}"),
        format!("suite_ply_min_max: {} {}", suite.ply_min, suite.ply_max),
        format!("suite_mean_snake_length: {:.2}", suite.mean_snake_length),
        format!("suite_positions_with_growth: {}", suite.with_growth),
        format!("suite_mean_pellets_left: {:.2}", suite.mean_pellets_left),
        format!("throughput_joint_moves_timed: {}", throughput.joint_moves),
        format!(
            "kernel_ns_per_advance: {:.1}",
            throughput.kernel_ns_per_advance
        ),
        format!(
            "kernel_advances_per_second: {:.0}",
            1e9 / throughput.kernel_ns_per_advance
        ),
        format!(
            "reference_ns_per_resolve: {:.1}",
            throughput.reference_ns_per_resolve
        ),
        format!(
            "reference_resolves_per_second: {:.0}",
            1e9 / throughput.reference_ns_per_resolve
        ),
        format!(
            "kernel_speedup: {:.1}x",
            throughput.reference_ns_per_resolve / throughput.kernel_ns_per_advance
        ),
        "search_allowance_ms: 370".to_owned(),
        format!("search_positions_searched: {}", search.depths.len()),
        format!(
            "search_completed_depth_min_median_max: {} {} {}",
            depth.min, depth.median, depth.max
        ),
        format!("search_completed_depth_mean: {mean_depth:.2}"),
        format!(
            "search_decisive_early_stops: {}",
            search.decisive_early_stops
        ),
        format!("search_nodes_total: {}", search.nodes_total),
        format!(
            "search_nodes_per_second_aggregate: {:.0}",
            search.nodes_per_second_aggregate
        ),
        format!(
            "search_nodes_per_second_median_position: {:.0}",
            search.nodes_per_second_median
        ),
        format!("search_slowest_decision_ms: {:.1}", search.slowest_ms),
        format!("repeat_depth_cap: {}", repeats.depth_cap),
        format!(
            "repeat_positions_sampled: {}",
            repeats.usable_percent_per_position.len()
        ),
        format!("repeat_final_iteration_nodes: {}", last.nodes),
        format!(
            "repeat_final_iteration_raw_percent: {:.2}",
            percent(last.raw_repeats, last.nodes)
        ),
        format!(
            "repeat_final_iteration_usable_percent: {:.2}",
            percent(last.usable_repeats, last.nodes)
        ),
        format!("repeat_whole_run_nodes: {}", run.nodes),
        format!(
            "repeat_whole_run_raw_percent: {:.2}",
            percent(run.raw_repeats, run.nodes)
        ),
        format!(
            "repeat_whole_run_usable_percent: {:.2}",
            percent(run.usable_repeats, run.nodes)
        ),
        format!(
            "repeat_usable_percent_median_position: {:.2}",
            median_f64(&repeats.usable_percent_per_position)
        ),
        format!(
            "repeat_positions_at_or_above_gate: {at_gate} of {}",
            repeats.usable_percent_per_position.len()
        ),
        format!("transposition_gate_percent: {REPEAT_GATE_PERCENT}"),
        format!(
            "transposition_gate: {}",
            if gate_passes(last) {
                "PASSED"
            } else {
                "NOT PASSED"
            }
        ),
        "=== end profile report ===".to_owned(),
    ];
    lines.join("\n")
}

/// Run by `scripts/run-profile` in release mode; prints the report the evidence
/// file quotes.
#[test]
#[ignore = "release-mode measurement, run through scripts/run-profile"]
fn profile_the_search_core() {
    let profile = measure_profile();

    println!("{}", render(&profile));

    assert_eq!(problems(&profile), Vec::<String>::new());
}

fn sample_profile() -> Profile {
    Profile {
        suite: SuiteFacts {
            size: SUITE_SIZE,
            ply_min: 20,
            ply_max: 45,
            mean_snake_length: 8.0,
            with_growth: 200,
            mean_pellets_left: 3.0,
        },
        throughput: Throughput {
            joint_moves: 16,
            kernel_ns_per_advance: 50.0,
            reference_ns_per_resolve: 150.0,
        },
        search: SearchFacts {
            depths: vec![3, 5, 9],
            decisive_early_stops: 1,
            nodes_total: 1000,
            nodes_per_second_aggregate: 1e6,
            nodes_per_second_median: 1e6,
            slowest_ms: 370.0,
        },
        repeats: RepeatFacts {
            depth_cap: 9,
            final_iteration: RepeatStats {
                nodes: 1000,
                raw_repeats: 200,
                usable_repeats: 150,
            },
            whole_run: RepeatStats::default(),
            usable_percent_per_position: vec![10.0, 20.0],
        },
    }
}

#[test]
fn a_sane_profile_has_no_problems_and_the_report_states_the_gate() {
    let profile = sample_profile();

    assert!(problems(&profile).is_empty());
    let text = render(&profile);
    assert!(text.contains("transposition_gate: PASSED"), "{text}");
    assert!(text.contains("kernel_speedup: 3.0x"), "{text}");
    assert!(
        text.contains("search_completed_depth_min_median_max: 3 5 9"),
        "{text}"
    );
}

#[test]
fn an_untrustworthy_profile_is_reported_not_rendered_as_fine() {
    let mut profile = sample_profile();
    profile.suite.size = 199;
    profile.search.depths.push(0);
    profile.throughput.kernel_ns_per_advance = 0.0;
    profile.repeats.final_iteration = RepeatStats::default();

    assert_eq!(problems(&profile).len(), 4);
    let mut low = sample_profile();
    low.repeats.final_iteration.usable_repeats = 149;
    assert!(render(&low).contains("transposition_gate: NOT PASSED"));
}
