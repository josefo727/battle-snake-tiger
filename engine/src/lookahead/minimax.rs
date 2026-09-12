//! Fixed-depth alpha-beta over the duel's two move layers.
//!
//! Each ply is a maximizing layer over our four headings followed by a
//! minimizing layer over the opponent's four, then both moves are applied at once
//! (ADR 0003's conservative serialization: we commit first, they answer knowing
//! it). Finished duels are scored by [`Finish`], every other leaf by the pipeline.
//! The search is fail-soft: a cut-off layer returns the bound it proved.

use super::ledger::LookaheadReport;
use crate::arena::duel::{Advance, DuelBoard};
use crate::arena::heading::Heading;
use crate::valuation::finish::Finish;
use crate::valuation::{AssessorSet, ValuationPipeline};

pub struct Searcher<'pipeline, S> {
    pipeline: &'pipeline ValuationPipeline<S>,
    finish: Finish,
    nodes: u64,
}

impl<'pipeline, S: AssessorSet> Searcher<'pipeline, S> {
    #[must_use]
    pub const fn new(pipeline: &'pipeline ValuationPipeline<S>, finish: Finish) -> Self {
        Self {
            pipeline,
            finish,
            nodes: 0,
        }
    }

    /// Searches exactly `depth` plies (each a full joint move) and reports the
    /// best heading and its exact minimax value. Ties go to the earliest heading
    /// in [`Heading::ALL`].
    ///
    /// # Panics
    ///
    /// Panics when `depth` is zero.
    pub fn search_fixed(&mut self, board: &DuelBoard, depth: u16) -> LookaheadReport {
        assert!(depth >= 1, "a search needs at least one ply");
        let start_nodes = self.nodes;
        let sentinel = self.finish.sentinel();
        let (best, score) = self.best_heading(board, depth, 0, -sentinel, sentinel);
        LookaheadReport::completed(depth, self.nodes - start_nodes, best, score)
    }

    /// The maximizing layer: our best heading for `board` and its value. The
    /// root reads the heading; interior plies only need the value.
    fn best_heading(
        &mut self,
        board: &DuelBoard,
        depth: u16,
        ply: u16,
        mut alpha: i32,
        beta: i32,
    ) -> (Heading, i32) {
        let mut best = (Heading::ALL[0], -self.finish.sentinel());
        for ours in Heading::ALL {
            let score = self.minimizer(board, ours, depth, ply, alpha, beta);
            if score > best.1 {
                best = (ours, score);
                alpha = alpha.max(score);
            }
            if alpha >= beta {
                break;
            }
        }
        best
    }

    /// The minimizing layer: the opponent's best reply to `ours`, each reply
    /// applied together with `ours` and valued one ply deeper.
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
        for theirs in Heading::ALL {
            self.nodes += 1;
            let score = match board.advance(ours, theirs) {
                Advance::Over(verdict) => self.finish.score(verdict, ply + 1),
                Advance::Continues(next) if depth == 1 => self.pipeline.score(&next),
                Advance::Continues(next) => {
                    self.best_heading(&next, depth - 1, ply + 1, alpha, beta).1
                }
            };
            best = best.min(score);
            beta = beta.min(best);
            if best <= alpha {
                break;
            }
        }
        best
    }
}
