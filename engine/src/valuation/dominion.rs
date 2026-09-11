//! Territory: which cells each serpent can reach strictly before the other.

use core::cmp::Ordering;

use super::Assessor;
use crate::arena::cellset::{Cell, CellSet};
use crate::arena::duel::{DuelBoard, Side};
use crate::arena::serpent::Serpent;

/// The cells each serpent owns. A cell reached at the same distance by both
/// belongs to the longer serpent, and to nobody when lengths are equal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Partition {
    pub ours: CellSet,
    pub theirs: CellSet,
}

/// A partition plus how many turns each serpent needs to reach the nearest
/// pellet it owns (`None` when it owns no pellet).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Survey {
    pub partition: Partition,
    pub our_food: Option<u16>,
    pub their_food: Option<u16>,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Dominion;

/// Fill steps to run before giving up. A cell frees by turn 121 at the latest
/// and a path crosses at most 120 cells afterwards, so 250 always suffices.
pub const MAX_LAYERS: u16 = 250;

impl Dominion {
    /// The partition together with each serpent's distance to its nearest
    /// owned pellet.
    #[must_use]
    pub fn survey(&self, board: &DuelBoard) -> Survey {
        self.fill(board)
    }

    /// Splits the cells between the serpents, modelling that bodies move away.
    ///
    /// Each serpent's fill advances one layer per turn. A body cell is passable
    /// from the turn its last segment leaves it (the segment `i` places from the
    /// tail frees after `i + 1` turns; a stacked copy delays it one turn each).
    /// A cell belongs to whoever arrives first, the longer serpent on a same-turn
    /// arrival, and nobody at equal length. Both start cells are never territory.
    #[must_use]
    pub fn partition(&self, board: &DuelBoard) -> Partition {
        self.fill(board).partition
    }

    fn fill(&self, board: &DuelBoard) -> Survey {
        let us = board.serpent(Side::Us);
        let them = board.serpent(Side::Them);
        let tie_winner = us.length().cmp(&them.length());
        let last_release = u16::from(us.length().max(them.length()));

        let mut free = board.occupied().complement();
        let mut our_fill = Fill::from(us.head());
        let mut their_fill = Fill::from(them.head());
        let mut decided = our_fill.seen.union(their_fill.seen);
        let mut partition = Partition {
            ours: CellSet::EMPTY,
            theirs: CellSet::EMPTY,
        };
        let mut our_food = None;
        let mut their_food = None;

        for layer in 1..=MAX_LAYERS {
            let released = released_at(us, layer).union(released_at(them, layer));
            free = free.union(released);
            let fresh_ours = our_fill.advance(free, released).difference(decided);
            let fresh_theirs = their_fill.advance(free, released).difference(decided);

            let (ours_now, theirs_now) = settle(fresh_ours, fresh_theirs, tie_winner);
            partition.ours = partition.ours.union(ours_now);
            partition.theirs = partition.theirs.union(theirs_now);
            decided = decided.union(fresh_ours).union(fresh_theirs);

            // A pellet inside newly owned cells is the nearest one owned so far.
            if our_food.is_none() && !ours_now.intersection(board.pellets()).is_empty() {
                our_food = Some(layer);
            }
            if their_food.is_none() && !theirs_now.intersection(board.pellets()).is_empty() {
                their_food = Some(layer);
            }

            if layer >= last_release && our_fill.is_exhausted() && their_fill.is_exhausted() {
                break;
            }
        }

        Survey {
            partition,
            our_food,
            their_food,
        }
    }
}

/// Ownership of the cells first reached this turn: each serpent keeps the cells
/// only it reached; cells both reached go to the longer serpent, or to nobody.
fn settle(fresh_ours: CellSet, fresh_theirs: CellSet, tie_winner: Ordering) -> (CellSet, CellSet) {
    let tied = fresh_ours.intersection(fresh_theirs);
    let ours = fresh_ours.difference(tied);
    let theirs = fresh_theirs.difference(tied);
    match tie_winner {
        Ordering::Greater => (ours.union(tied), theirs),
        Ordering::Less => (ours, theirs.union(tied)),
        Ordering::Equal => (ours, theirs),
    }
}

/// One serpent's fill: the cells reached on the latest turn and every cell
/// reached so far.
#[derive(Clone, Copy)]
struct Fill {
    front: CellSet,
    seen: CellSet,
}

impl Fill {
    fn from(head: Cell) -> Self {
        let start = CellSet::single(head);
        Self {
            front: start,
            seen: start,
        }
    }

    /// One turn: step into free unseen neighbours, and enter cells that freed
    /// this turn beside ground already reached (the serpent can dawdle there
    /// until they free). Returns the cells newly reached.
    fn advance(&mut self, free: CellSet, released: CellSet) -> CellSet {
        let by_walking = self.front.neighbours();
        let by_waiting = released.intersection(self.seen.neighbours());
        self.front = by_walking
            .union(by_waiting)
            .intersection(free)
            .difference(self.seen);
        self.seen = self.seen.union(self.front);
        self.front
    }

    const fn is_exhausted(&self) -> bool {
        self.front.is_empty()
    }
}

/// The cell a serpent's body vacates entirely on turn `layer`, if any.
fn released_at(serpent: &Serpent, layer: u16) -> CellSet {
    let index = layer - 1;
    if index >= u16::from(serpent.length()) {
        return CellSet::EMPTY;
    }
    let index = index as u8;
    let cell = serpent.cell_from_tail(index);
    let held_by_a_later_copy =
        index + 1 < serpent.length() && serpent.cell_from_tail(index + 1) == cell;
    if held_by_a_later_copy {
        CellSet::EMPTY
    } else {
        CellSet::single(cell)
    }
}

impl Assessor for Dominion {
    const NAME: &'static str = "dominion";

    fn assess(&self, board: &DuelBoard) -> i32 {
        let partition = self.partition(board);
        partition.ours.len().cast_signed() - partition.theirs.len().cast_signed()
    }
}
