//! Fixed-depth alpha-beta over the duel's two move layers.
//!
//! Each ply is a maximizing layer over our four headings followed by a
//! minimizing layer over the opponent's four, then both moves are applied at once
//! (ADR 0003's conservative serialization: we commit first, they answer knowing
//! it). Finished duels are scored by [`Finish`], every other leaf by the pipeline.
//! The search is fail-soft: a cut-off layer returns the bound it proved.

use super::allowance::{NeverStop, StopSignal};
use super::ledger::LookaheadReport;
use super::ordering::{HeadingOrder, LearnedOrder};
use crate::arena::duel::{Advance, DuelBoard, Side};
use crate::arena::heading::Heading;
use crate::valuation::finish::Finish;
use crate::valuation::{AssessorSet, ValuationPipeline};

/// The alpha-beta bounds a layer searches inside.
#[derive(Clone, Copy, Debug)]
pub(super) struct Window {
    pub(super) alpha: i32,
    pub(super) beta: i32,
}

impl Window {
    /// Everything between the two sentinels: no bound known yet.
    pub(super) const fn open(sentinel: i32) -> Self {
        Self {
            alpha: -sentinel,
            beta: sentinel,
        }
    }
}

/// A search cut short by its stop signal; nothing partial survives it.
#[derive(Clone, Copy, Debug)]
pub(super) struct Interrupted;

pub struct Searcher<'pipeline, S, O = LearnedOrder> {
    pipeline: &'pipeline ValuationPipeline<S>,
    finish: Finish,
    order: O,
    nodes: u64,
}

impl<'pipeline, S: AssessorSet> Searcher<'pipeline, S> {
    /// A searcher that learns its move order as it goes.
    #[must_use]
    pub fn new(pipeline: &'pipeline ValuationPipeline<S>, finish: Finish) -> Self {
        Self::with_order(pipeline, finish, LearnedOrder::new())
    }
}

impl<'pipeline, S: AssessorSet, O: HeadingOrder> Searcher<'pipeline, S, O> {
    #[must_use]
    pub const fn with_order(
        pipeline: &'pipeline ValuationPipeline<S>,
        finish: Finish,
        order: O,
    ) -> Self {
        Self {
            pipeline,
            finish,
            order,
            nodes: 0,
        }
    }

    /// Searches exactly `depth` plies (each a full joint move) and reports the
    /// best heading and its exact minimax value. Ties go to the earliest heading
    /// in [`Heading::ALL`], whatever order the layers are tried in.
    ///
    /// # Panics
    ///
    /// Panics when `depth` is zero.
    pub fn search_fixed(&mut self, board: &DuelBoard, depth: u16) -> LookaheadReport {
        self.search_until(board, depth, &mut NeverStop)
            .expect("a search that is never told to stop completes")
    }

    /// Like [`Self::search_fixed`] but consults `stop` once per node and gives up
    /// with `None` as soon as it says stop, leaving no partial answer behind. The
    /// learned order only ever records genuine cutoffs, so an abandoned search
    /// cannot make the next one wrong, only differently ordered.
    ///
    /// # Panics
    ///
    /// Panics when `depth` is zero.
    pub fn search_until(
        &mut self,
        board: &DuelBoard,
        depth: u16,
        stop: &mut impl StopSignal,
    ) -> Option<LookaheadReport> {
        assert!(depth >= 1, "a search needs at least one ply");
        let start_nodes = self.nodes;
        let sentinel = self.finish.sentinel();
        let (best, score) = self
            .best_heading(board, depth, 0, Window::open(sentinel), stop)
            .ok()?;
        self.order.note_root_best(best);
        Some(LookaheadReport::completed(
            depth,
            self.nodes - start_nodes,
            best,
            score,
        ))
    }

    /// Every position visited by this searcher so far, interrupted searches
    /// included.
    #[must_use]
    pub const fn nodes_visited(&self) -> u64 {
        self.nodes
    }

    /// Whether `score` is a finished game rather than a positional estimate, so
    /// searching deeper cannot change it.
    #[must_use]
    pub const fn is_decisive(&self, score: i32) -> bool {
        score.abs() >= self.finish.finite_limit()
    }

    /// The maximizing layer: our best heading for `board` and its value. The
    /// root reads the heading; interior plies only need the value.
    fn best_heading(
        &mut self,
        board: &DuelBoard,
        depth: u16,
        ply: u16,
        window: Window,
        stop: &mut impl StopSignal,
    ) -> Result<(Heading, i32), Interrupted> {
        let Window { mut alpha, beta } = window;
        let mut best = (Heading::ALL[0], -self.finish.sentinel());
        let root = ply == 0;
        for ours in self.order.arrange(Side::Us, ply) {
            // At the root an earlier heading must win an exact tie whatever the
            // trial order, so it is searched one point wider to tell a tie from
            // a bound.
            let wins_ties = root && ours.index() < best.0.index();
            let floor = if wins_ties { alpha - 1 } else { alpha };
            let narrowed = Window { alpha: floor, beta };
            let score = self.minimizer(board, ours, depth, ply, narrowed, stop)?;
            if score > best.1 || (wins_ties && score == best.1) {
                best = (ours, score);
                alpha = alpha.max(score);
            }
            if alpha >= beta {
                self.order.note_cutoff(Side::Us, ply, ours, depth);
                break;
            }
        }
        Ok(best)
    }

    /// The minimizing layer: the opponent's best reply to `ours`, each reply
    /// applied together with `ours` and valued one ply deeper.
    fn minimizer(
        &mut self,
        board: &DuelBoard,
        ours: Heading,
        depth: u16,
        ply: u16,
        window: Window,
        stop: &mut impl StopSignal,
    ) -> Result<i32, Interrupted> {
        let Window { alpha, mut beta } = window;
        let mut best = self.finish.sentinel();
        for theirs in self.order.arrange(Side::Them, ply) {
            if stop.should_stop() {
                return Err(Interrupted);
            }
            self.nodes += 1;
            let score = match board.advance(ours, theirs) {
                Advance::Over(verdict) => self.finish.score(verdict, ply + 1),
                Advance::Continues(next) if depth == 1 => self.pipeline.score(&next),
                Advance::Continues(next) => {
                    let inner = Window { alpha, beta };
                    self.best_heading(&next, depth - 1, ply + 1, inner, stop)?.1
                }
            };
            best = best.min(score);
            beta = beta.min(best);
            if best <= alpha {
                self.order.note_cutoff(Side::Them, ply, theirs, depth);
                break;
            }
        }
        Ok(best)
    }
}
