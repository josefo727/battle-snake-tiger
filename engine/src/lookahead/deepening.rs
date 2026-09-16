//! Iterative deepening: search one ply deeper until the allowance says stop.
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

/// A new depth starts only while less than this share of the allowance, in
/// percent, has elapsed. Deeper iterations cost several times the previous one,
/// so starting late mostly wastes the time that remains.
pub const ITERATION_START_PERCENT: u64 = 40;

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
    let mut span = None;

    for depth in 1..=depth_limit.min(DEPTH_CEILING) {
        let left = allowance.time_left();
        let span = *span.get_or_insert(left);
        if !may_start_iteration(left, span) {
            break;
        }
        let Some(completed) = searcher.search_until(depth, allowance) else {
            break;
        };
        let decisive = completed
            .principal_score
            .is_some_and(|s| searcher.is_decisive(s));
        deepest = Some(completed);
        if decisive {
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

/// Whether a new iteration may begin with `left` of an allowance that was `span`
/// long when the search began: only while under [`ITERATION_START_PERCENT`] of it
/// has elapsed, and never once nothing is left.
fn may_start_iteration(left: Duration, span: Duration) -> bool {
    let elapsed = span.saturating_sub(left);
    elapsed.as_micros() * 100 < span.as_micros() * u128::from(ITERATION_START_PERCENT)
}
