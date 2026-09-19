//! The melee search split at the root over threads (growth iteration 7): each
//! root heading is valued in its own lane with an open window, lanes are shared
//! out over `threads`, and the best heading is the greatest value, earliest
//! heading on a tie. Every lane keeps its own move-order tables and its own
//! allowance on the same deadline, so nothing is shared between threads but
//! the position and the valuation.

use std::thread;

use super::allowance::{SearchAllowance, StopSignal};
use super::deepening::IterativeSearch;
use super::ledger::LookaheadReport;
use super::paranoid::MeleeSearcher;
use crate::arena::heading::Heading;
use crate::arena::melee::MeleeBoard;
use crate::rules_core::{Clock, MonotonicInstant};
use crate::valuation::melee::MeleeValuation;
use crate::valuation::melee::finish::MeleeFinish;

/// One root heading and the searcher that values it, iteration after iteration.
struct Lane<'valuation> {
    heading: Heading,
    searcher: MeleeSearcher<'valuation>,
}

pub struct RootSplit<'valuation, 'clock> {
    board: MeleeBoard,
    lanes: Vec<Lane<'valuation>>,
    clock: &'clock dyn Clock,
    deadline: MonotonicInstant,
    threads: usize,
    finish: MeleeFinish,
}

impl<'valuation, 'clock> RootSplit<'valuation, 'clock> {
    /// A split search of `board` over at most `threads` threads (at least one),
    /// stopping at `deadline` (the search deadline, tail margin already taken).
    #[must_use]
    pub fn new(
        valuation: &'valuation MeleeValuation,
        finish: MeleeFinish,
        board: MeleeBoard,
        clock: &'clock dyn Clock,
        deadline: MonotonicInstant,
        threads: usize,
    ) -> Self {
        Self {
            board,
            lanes: Heading::ALL
                .into_iter()
                .map(|heading| Lane {
                    heading,
                    searcher: MeleeSearcher::new(valuation, finish),
                })
                .collect(),
            clock,
            deadline,
            threads: threads.max(1),
            finish,
        }
    }

    /// Searches exactly `depth` plies in every lane, `threads` lanes at a time,
    /// and reports the best heading; `None` as soon as any lane is stopped.
    pub fn search_fixed_split(&mut self, depth: u16) -> Option<LookaheadReport> {
        let start_nodes = self.nodes_visited();
        let board = self.board;
        let (clock, deadline, threads) = (self.clock, self.deadline, self.threads);
        let mut lanes: Vec<&mut Lane<'valuation>> = self.lanes.iter_mut().collect();
        let per_thread = lanes.len().div_ceil(threads);
        let values: Vec<Option<i32>> = thread::scope(|scope| {
            let workers: Vec<_> = lanes
                .chunks_mut(per_thread)
                .map(|group| {
                    scope.spawn(move || {
                        let mut stop = SearchAllowance::with_deadline(clock, deadline);
                        group
                            .iter_mut()
                            .map(|lane| {
                                lane.searcher
                                    .root_value(&board, lane.heading, depth, &mut stop)
                            })
                            .collect::<Vec<Option<i32>>>()
                    })
                })
                .collect();
            workers
                .into_iter()
                .flat_map(|worker| worker.join().expect("a lane never panics"))
                .collect()
        });
        let mut best: Option<(Heading, i32)> = None;
        for (lane, value) in self.lanes.iter().zip(values) {
            let value = value?;
            if best.is_none_or(|(_, score)| value > score) {
                best = Some((lane.heading, value));
            }
        }
        let (heading, score) = best?;
        Some(LookaheadReport::completed(
            depth,
            self.nodes_visited() - start_nodes,
            heading,
            score,
        ))
    }
}

impl IterativeSearch for RootSplit<'_, '_> {
    /// The driver's stop signal is not shared between threads: every lane polls
    /// its own allowance on the same deadline.
    fn search_until(&mut self, depth: u16, _stop: &mut impl StopSignal) -> Option<LookaheadReport> {
        self.search_fixed_split(depth)
    }

    fn nodes_visited(&self) -> u64 {
        self.lanes
            .iter()
            .map(|lane| lane.searcher.nodes_visited())
            .sum()
    }

    fn is_decisive(&self, score: i32) -> bool {
        score.abs() >= self.finish.finite_limit()
    }
}
