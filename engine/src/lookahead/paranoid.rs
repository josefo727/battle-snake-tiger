//! Fixed-depth paranoid alpha-beta over the seats of a melee (ADR 0007).
//!
//! Each ply is a maximizing layer over our four headings followed by one
//! minimizing layer per living opponent, in seat order, then every move is
//! applied at once: every opponent is assumed to play against us, knowing our
//! move and the replies before it. Finished melees are scored by
//! [`MeleeFinish`], every other leaf by [`MeleeValuation`]. Fail-soft, like the
//! duel searcher.

use super::allowance::{NeverStop, StopSignal};
use super::ledger::LookaheadReport;
use super::minimax::{Interrupted, Window};
use super::ordering::{HeadingOrder, LearnedOrder};
use crate::arena::heading::Heading;
use crate::arena::melee::{MAX_SEATS, MeleeBoard, MeleeOutcome, Seat};
use crate::valuation::melee::MeleeValuation;
use crate::valuation::melee::finish::MeleeFinish;

pub struct MeleeSearcher<'valuation, O = LearnedOrder> {
    valuation: &'valuation MeleeValuation,
    finish: MeleeFinish,
    order: O,
    nodes: u64,
}

impl<'valuation> MeleeSearcher<'valuation> {
    /// A searcher that learns its move order as it goes.
    #[must_use]
    pub fn new(valuation: &'valuation MeleeValuation, finish: MeleeFinish) -> Self {
        Self::with_order(valuation, finish, LearnedOrder::new())
    }
}

impl<'valuation, O: HeadingOrder> MeleeSearcher<'valuation, O> {
    #[must_use]
    pub const fn with_order(
        valuation: &'valuation MeleeValuation,
        finish: MeleeFinish,
        order: O,
    ) -> Self {
        Self {
            valuation,
            finish,
            order,
            nodes: 0,
        }
    }

    /// Searches exactly `depth` plies (each a full joint move of every living
    /// seat) and reports the best heading and its exact paranoid value. Ties go
    /// to the earliest heading in [`Heading::ALL`].
    ///
    /// # Panics
    ///
    /// Panics when `depth` is zero.
    pub fn search_fixed(&mut self, board: &MeleeBoard, depth: u16) -> LookaheadReport {
        self.search_until(board, depth, &mut NeverStop)
            .expect("a search that is never told to stop completes")
    }

    /// Like [`Self::search_fixed`] but consults `stop` once per joint move and
    /// gives up with `None` as soon as it says stop, leaving nothing partial.
    ///
    /// # Panics
    ///
    /// Panics when `depth` is zero.
    pub fn search_until(
        &mut self,
        board: &MeleeBoard,
        depth: u16,
        stop: &mut impl StopSignal,
    ) -> Option<LookaheadReport> {
        assert!(depth >= 1, "a search needs at least one ply");
        let start_nodes = self.nodes;
        let sentinel = self.finish.sentinel();
        let (best, score) = self
            .maximize(board, depth, 0, Window::open(sentinel), stop)
            .ok()?;
        self.order.note_root_best(best);
        Some(LookaheadReport::completed(
            depth,
            self.nodes - start_nodes,
            best,
            score,
        ))
    }

    /// The maximizing layer: our best heading for `board` and its value. The
    /// root reads the heading; interior plies only need the value.
    fn maximize(
        &mut self,
        board: &MeleeBoard,
        depth: u16,
        ply: u16,
        window: Window,
        stop: &mut impl StopSignal,
    ) -> Result<(Heading, i32), Interrupted> {
        let Window { mut alpha, beta } = window;
        let mut best = (Heading::ALL[0], -self.finish.sentinel());
        let root = ply == 0;
        for ours in self.order.arrange(Seat::US, ply) {
            // At the root an earlier heading must win an exact tie whatever the
            // trial order, so it is searched one point wider to tell a tie from
            // a bound (the duel searcher does the same).
            let wins_ties = root && ours.index() < best.0.index();
            let floor = if wins_ties { alpha - 1 } else { alpha };
            let mut chosen = [ours; MAX_SEATS];
            let score = self.minimize(
                board,
                &mut chosen,
                1,
                depth,
                ply,
                Window { alpha: floor, beta },
                stop,
            )?;
            if score > best.1 || (wins_ties && score == best.1) {
                best = (ours, score);
                alpha = alpha.max(score);
            }
            if alpha >= beta {
                self.order.note_cutoff(Seat::US, ply, ours, depth);
                break;
            }
        }
        Ok(best)
    }

    /// The minimizing layers: the reply of the next living opponent from seat
    /// index `from`, each applied together with everything chosen so far. Once
    /// every opponent has chosen, the joint move is played and valued.
    #[allow(clippy::too_many_arguments)]
    fn minimize(
        &mut self,
        board: &MeleeBoard,
        chosen: &mut [Heading; MAX_SEATS],
        from: usize,
        depth: u16,
        ply: u16,
        window: Window,
        stop: &mut impl StopSignal,
    ) -> Result<i32, Interrupted> {
        let Some(seat) = (from..MAX_SEATS)
            .map(|index| Seat::ALL[index])
            .find(|seat| board.is_alive(*seat))
        else {
            return self.play(board, chosen, depth, ply, window, stop);
        };
        let Window { alpha, mut beta } = window;
        let mut best = self.finish.sentinel();
        for theirs in self.order.arrange(seat, ply) {
            chosen[seat.index()] = theirs;
            let inner = Window { alpha, beta };
            let score = self.minimize(board, chosen, seat.index() + 1, depth, ply, inner, stop)?;
            best = best.min(score);
            beta = beta.min(best);
            if best <= alpha {
                self.order.note_cutoff(seat, ply, theirs, depth);
                break;
            }
        }
        Ok(best)
    }

    /// One joint move: the finished melee's terminal score, the leaf value, or
    /// the next ply's maximizing layer.
    fn play(
        &mut self,
        board: &MeleeBoard,
        chosen: &[Heading; MAX_SEATS],
        depth: u16,
        ply: u16,
        window: Window,
        stop: &mut impl StopSignal,
    ) -> Result<i32, Interrupted> {
        if stop.should_stop() {
            return Err(Interrupted);
        }
        self.nodes += 1;
        Ok(match board.advance(chosen) {
            MeleeOutcome::Continues(next) if depth == 1 => self.valuation.score(&next),
            MeleeOutcome::Continues(next) => {
                self.maximize(&next, depth - 1, ply + 1, window, stop)?.1
            }
            ended => self.finish.score(&ended, ply + 1),
        })
    }

    /// Every position visited by this searcher so far, interrupted searches
    /// included.
    #[must_use]
    pub const fn nodes_visited(&self) -> u64 {
        self.nodes
    }

    /// Whether `score` is a finished game rather than a positional estimate.
    #[must_use]
    pub const fn is_decisive(&self, score: i32) -> bool {
        score.abs() >= self.finish.finite_limit()
    }
}
