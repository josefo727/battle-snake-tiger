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
use crate::arena::cellset::{Cell, CellSet};
use crate::arena::heading::Heading;
use crate::arena::melee::{MAX_SEATS, MeleeBoard, MeleeOutcome, Seat};
use crate::valuation::melee::MeleeValuation;
use crate::valuation::melee::finish::MeleeFinish;

/// What a root heading pays for entering a cell that an equal-length rival's
/// head can also enter this turn. The opponent model assumes such a rival does
/// not trade heads (growth iteration 4); the platform's population sometimes
/// does, so with a free alternative of similar value the root avoids the coin
/// flip, and with a clearly worse alternative it still takes the cell.
pub const TRADE_RISK: i32 = 8_000;

pub struct MeleeSearcher<'valuation, O = LearnedOrder> {
    valuation: &'valuation MeleeValuation,
    finish: MeleeFinish,
    order: O,
    nodes: u64,
    prune_opponents: bool,
    trade_risk: i32,
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
            prune_opponents: true,
            trade_risk: TRADE_RISK,
        }
    }

    /// A root heading into a cell an equal-length rival can also enter pays
    /// `risk` (the default is [`TRADE_RISK`]; zero restores the plain model).
    #[must_use]
    pub const fn with_trade_risk(mut self, risk: i32) -> Self {
        self.trade_risk = risk;
        self
    }

    /// Below the root, opponents normally try only the headings that do not
    /// kill them outright; this keeps all four everywhere (for measurements).
    #[must_use]
    pub const fn without_opponent_pruning(mut self) -> Self {
        self.prune_opponents = false;
        self
    }

    /// Searches exactly `depth` plies (each a full joint move of every living
    /// seat) and reports the best heading and its exact paranoid value. Ties go
    /// to the earliest heading in [`Heading::ALL`] that does not kill us by itself.
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
        let fatal = if root {
            self_fatal_headings(board)
        } else {
            HeadingSet::of(|_| false)
        };
        for ours in self.order.arrange(Seat::US, ply) {
            // At the root an exact tie goes to the heading that precedes the
            // other whatever the trial order (a heading that kills us by itself
            // comes after every other, then the fixed order), so it is searched
            // one point wider to tell a tie from a bound (the duel searcher does
            // the same).
            let wins_ties = root && precedes(ours, best.0, fatal);
            let floor = if wins_ties { alpha - 1 } else { alpha };
            let mut chosen = [ours; MAX_SEATS];
            let our_target = ours.step(board.serpent(Seat::US).head());
            let risk = if root {
                self.root_risk(board, our_target)
            } else {
                0
            };
            let score = self
                .minimize(
                    board,
                    &mut chosen,
                    our_target,
                    1,
                    depth,
                    ply,
                    Window {
                        alpha: floor.saturating_add(risk),
                        beta: beta.saturating_add(risk),
                    },
                    stop,
                )?
                .saturating_sub(risk);
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
        our_target: Option<Cell>,
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
        let considered = self.replies_of(board, seat, ply, our_target);
        for theirs in self.order.arrange(seat, ply) {
            if !considered.contains(&theirs) {
                continue;
            }
            chosen[seat.index()] = theirs;
            let inner = Window { alpha, beta };
            let score = self.minimize(
                board,
                chosen,
                our_target,
                seat.index() + 1,
                depth,
                ply,
                inner,
                stop,
            )?;
            best = best.min(score);
            beta = beta.min(best);
            if best <= alpha {
                self.order.note_cutoff(seat, ply, theirs, depth);
                break;
            }
        }
        Ok(best)
    }

    /// The headings an opponent tries (the opponent model, ADR 0007):
    /// - without pruning, or at the root, every heading; below the root only
    ///   those that do not kill it outright (off the board or into a body cell
    ///   that stays), unless it has none, when it keeps all four;
    /// - at every ply, a rival no longer than us is not assumed to trade heads
    ///   with us (step into `our_target`, its own certain death for ours), unless
    ///   that is its only step that does not kill it outright: "nothing else" is
    ///   judged on the self-preserving headings at the root too, so a rival
    ///   boxed against the contested cell keeps the threat. A strictly longer
    ///   rival keeps the threat always.
    fn replies_of(
        &self,
        board: &MeleeBoard,
        seat: Seat,
        ply: u16,
        our_target: Option<Cell>,
    ) -> HeadingSet {
        if !self.prune_opponents {
            return HeadingSet::ALL;
        }
        let head = board.serpent(seat).head();
        let enterable = enterable_cells(board);
        let safe = HeadingSet::of(|heading| {
            heading
                .step(head)
                .is_some_and(|cell| enterable.contains(cell))
        });
        let judged = if safe.is_empty() {
            HeadingSet::ALL
        } else {
            safe
        };
        let preserving = if ply == 0 { HeadingSet::ALL } else { judged };
        let no_longer_than_us = board.serpent(seat).length() <= board.serpent(Seat::US).length();
        let Some(target) = our_target.filter(|_| no_longer_than_us) else {
            return preserving;
        };
        let has_untraded = Heading::ALL
            .into_iter()
            .any(|heading| judged.contains(&heading) && heading.step(head) != Some(target));
        if !has_untraded {
            return preserving;
        }
        HeadingSet::of(|heading| {
            preserving.contains(&heading) && heading.step(head) != Some(target)
        })
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

    /// The exact paranoid value of answering `ours` from `board` at `depth`
    /// plies (the root's minimizing layers with an open window), or `None` at
    /// the stop signal. One root heading's share of a search split over threads.
    ///
    /// # Panics
    ///
    /// Panics when `depth` is zero.
    pub fn root_value(
        &mut self,
        board: &MeleeBoard,
        ours: Heading,
        depth: u16,
        stop: &mut impl StopSignal,
    ) -> Option<i32> {
        assert!(depth >= 1, "a search needs at least one ply");
        let mut chosen = [ours; MAX_SEATS];
        let our_target = ours.step(board.serpent(Seat::US).head());
        let risk = self.root_risk(board, our_target);
        self.minimize(
            board,
            &mut chosen,
            our_target,
            1,
            depth,
            0,
            Window::open(self.finish.sentinel()),
            stop,
        )
        .ok()
        .map(|value| value.saturating_sub(risk))
    }

    /// What our root heading into `our_target` pays: [`Self::with_trade_risk`]
    /// when the cell is contested by an equal-length rival, nothing otherwise.
    fn root_risk(&self, board: &MeleeBoard, our_target: Option<Cell>) -> i32 {
        match our_target {
            Some(target) if contested_by_equal(board, target) => self.trade_risk,
            _ => 0,
        }
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

/// A subset of the four headings, as a bit per [`Heading::index`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct HeadingSet(u8);

impl HeadingSet {
    const ALL: Self = Self(0b1111);

    fn of(mut keep: impl FnMut(Heading) -> bool) -> Self {
        Self(Heading::ALL.into_iter().fold(0, |bits, heading| {
            bits | (u8::from(keep(heading)) << heading.index())
        }))
    }

    const fn contains(self, heading: &Heading) -> bool {
        self.0 >> heading.index() & 1 == 1
    }

    const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

/// Whether `a` wins an exact tie against `b` at the root: a heading that kills
/// us by itself (in `fatal`) loses to any that does not; otherwise the earlier
/// heading in [`Heading::ALL`] wins.
pub(super) fn precedes(a: Heading, b: Heading, fatal: HeadingSet) -> bool {
    (fatal.contains(&a), a.index()) < (fatal.contains(&b), b.index())
}

/// Whether a living rival of exactly our length can step into `target` this
/// turn: the head-to-head the opponent model assumes it declines.
#[must_use]
pub fn contested_by_equal(board: &MeleeBoard, target: Cell) -> bool {
    let our_length = board.serpent(Seat::US).length();
    board
        .seats()
        .filter(|seat| *seat != Seat::US && board.is_alive(*seat))
        .filter(|seat| board.serpent(*seat).length() == our_length)
        .any(|seat| {
            let head = board.serpent(seat).head();
            Heading::ALL
                .into_iter()
                .any(|heading| heading.step(head) == Some(target))
        })
}

/// Our headings that kill us with no rival's help: off the board or into a
/// cell that stays occupied this turn.
pub(super) fn self_fatal_headings(board: &MeleeBoard) -> HeadingSet {
    let head = board.serpent(Seat::US).head();
    let enterable = enterable_cells(board);
    HeadingSet::of(|heading| {
        !heading
            .step(head)
            .is_some_and(|cell| enterable.contains(cell))
    })
}

/// Free cells plus the tail cells that vacate this turn.
fn enterable_cells(board: &MeleeBoard) -> CellSet {
    board
        .seats()
        .fold(board.occupied().complement(), |cells, seat| {
            board
                .serpent(seat)
                .cell_released_on_turn(1)
                .map_or(cells, |cell| cells.with(cell))
        })
}
