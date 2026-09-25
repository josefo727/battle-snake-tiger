//! Iterative deepening: search one ply deeper while the next depth is predicted
//! to fit in the time left (growth iteration 12: the last iteration's duration
//! times the observed growth, in place of a fixed share of the allowance).
//!
//! Only a completed depth is ever reported; an interrupted iteration is dropped
//! and the last completed depth's heading stands (spec criterion 3).

use core::time::Duration;

use super::allowance::{SearchAllowance, StopSignal};
use super::ledger::LookaheadReport;
use super::minimax::Searcher;
use super::ordering::HeadingOrder;
use super::paranoid::MeleeSearcher;
use crate::arena::duel::DuelBoard;
use crate::arena::melee::MeleeBoard;
use crate::valuation::AssessorSet;

/// The growth of one iteration's cost over the previous one is assumed to lie
/// between these factors when predicting whether the next depth fits in the
/// time left; with one iteration measured the growth is taken as [`GROWTH_DEFAULT`].
pub const GROWTH_FLOOR: u32 = 2;
pub const GROWTH_CEILING: u32 = 8;
pub const GROWTH_DEFAULT: u32 = 4;

/// The deepest iteration ever attempted, however much time is left.
pub const DEPTH_CEILING: u16 = 64;

/// Runs depths 1, 2, 3, ... up to `depth_limit` (at most [`DEPTH_CEILING`]) on a
/// duel and reports the deepest one that completed, with the positions explored
/// across every iteration, the abandoned one included. Stops early when a depth's
/// answer is already a finished game, and reports that nothing completed when the
/// allowance leaves no room for even the first depth.
///
/// Reads the clock once per iteration boundary; the searcher's own polling covers
/// the time in between.
pub fn deepen<S: AssessorSet, O: HeadingOrder>(
    searcher: &mut Searcher<'_, S, O>,
    board: &DuelBoard,
    allowance: &mut SearchAllowance<'_>,
    depth_limit: u16,
) -> LookaheadReport {
    drive(&mut DuelSearch { searcher, board }, allowance, depth_limit)
}

/// [`deepen`] for a melee.
pub fn deepen_melee<O: HeadingOrder>(
    searcher: &mut MeleeSearcher<'_, O>,
    board: &MeleeBoard,
    allowance: &mut SearchAllowance<'_>,
    depth_limit: u16,
) -> LookaheadReport {
    drive(&mut MeleeSearch { searcher, board }, allowance, depth_limit)
}

/// A melee searcher bound to the position it searches.
struct MeleeSearch<'a, 'valuation, O> {
    searcher: &'a mut MeleeSearcher<'valuation, O>,
    board: &'a MeleeBoard,
}

impl<O: HeadingOrder> IterativeSearch for MeleeSearch<'_, '_, O> {
    fn search_until(&mut self, depth: u16, stop: &mut impl StopSignal) -> Option<LookaheadReport> {
        self.searcher.search_until(self.board, depth, stop)
    }

    fn nodes_visited(&self) -> u64 {
        self.searcher.nodes_visited()
    }

    fn is_decisive(&self, score: i32) -> bool {
        self.searcher.is_decisive(score)
    }
}

/// A duel searcher bound to the position it searches.
struct DuelSearch<'a, 'pipeline, S, O> {
    searcher: &'a mut Searcher<'pipeline, S, O>,
    board: &'a DuelBoard,
}

impl<S: AssessorSet, O: HeadingOrder> IterativeSearch for DuelSearch<'_, '_, S, O> {
    fn search_until(&mut self, depth: u16, stop: &mut impl StopSignal) -> Option<LookaheadReport> {
        self.searcher.search_until(self.board, depth, stop)
    }

    fn nodes_visited(&self) -> u64 {
        self.searcher.nodes_visited()
    }

    fn is_decisive(&self, score: i32) -> bool {
        self.searcher.is_decisive(score)
    }
}

/// One fixed-depth search the driver can repeat at growing depths.
pub trait IterativeSearch {
    /// Searches exactly `depth` plies, or gives up with `None` at the stop signal.
    fn search_until(&mut self, depth: u16, stop: &mut impl StopSignal) -> Option<LookaheadReport>;

    /// Every position visited so far, interrupted searches included.
    fn nodes_visited(&self) -> u64;

    /// Whether `score` is a finished game rather than a positional estimate.
    fn is_decisive(&self, score: i32) -> bool;
}

/// The deepening loop behind [`deepen`] and [`deepen_melee`].
pub fn drive(
    search: &mut impl IterativeSearch,
    allowance: &mut SearchAllowance<'_>,
    depth_limit: u16,
) -> LookaheadReport {
    let searcher = search;
    let baseline = searcher.nodes_visited();
    let mut deepest = None;
    let mut durations: Vec<Duration> = Vec::new();
    let mut left_at_start = None;

    for depth in 1..=depth_limit.min(DEPTH_CEILING) {
        let left = allowance.time_left();
        if let Some(started) = left_at_start {
            durations.push(started - left);
        }
        if left.is_zero() || predicted_next(&durations) > left {
            break;
        }
        left_at_start = Some(left);
        let Some(completed) = searcher.search_until(depth, allowance) else {
            break;
        };
        // A proven win stops the deepening: nothing deeper can better it. A
        // proven loss does not. The verdict is only as good as the opponent
        // model behind it, and that model has every rival play the worst line
        // for us every time; on the ladder on 2026-09-25 it called a melee lost
        // from turn 105 and the driver answered the next four turns in 45, 1.4,
        // 0.4 and 0.1 ms of a 500 ms budget, leaving the move to the fixed
        // heading order among headings it had declared equally lost. The
        // rollout reference put ten turns of life between the best of them and
        // the worst. Searching on costs time we would otherwise throw away, and
        // deeper plies at least prefer the line that dies latest.
        let proven_win = completed
            .principal_score
            .is_some_and(|score| score > 0 && searcher.is_decisive(score));
        deepest = Some(completed);
        if proven_win {
            break;
        }
    }

    let nodes = searcher.nodes_visited() - baseline;
    deepest.map_or_else(
        || LookaheadReport::nothing_completed(nodes),
        |report| LookaheadReport {
            nodes_explored: nodes,
            ..report
        },
    )
}

/// How long the next iteration is expected to take from the completed ones:
/// nothing before the first (depth 1 always starts), the last duration times
/// the observed growth (clamped to [`GROWTH_FLOOR`]..=[`GROWTH_CEILING`]) or
/// times [`GROWTH_DEFAULT`] when only one duration is known.
fn predicted_next(durations: &[Duration]) -> Duration {
    match durations {
        [] => Duration::ZERO,
        [only] => *only * GROWTH_DEFAULT,
        [.., previous, last] => {
            let growth = if previous.is_zero() {
                GROWTH_CEILING
            } else {
                let ratio = last.as_micros().div_ceil(previous.as_micros().max(1));
                u32::try_from(ratio)
                    .unwrap_or(GROWTH_CEILING)
                    .clamp(GROWTH_FLOOR, GROWTH_CEILING)
            };
            *last * growth
        }
    }
}
