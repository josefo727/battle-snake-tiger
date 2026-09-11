//! Territory: which cells each serpent can reach strictly before the other.

use core::cmp::Ordering;

use super::Assessor;
use crate::arena::cellset::CellSet;
use crate::arena::duel::{DuelBoard, Side};

/// The cells each serpent owns. A cell reached at the same distance by both
/// belongs to the longer serpent, and to nobody when lengths are equal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Partition {
    pub ours: CellSet,
    pub theirs: CellSet,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Dominion;

impl Dominion {
    /// Splits the free cells between the serpents using static obstacles (every
    /// serpent cell blocks for the whole search).
    ///
    /// Each serpent's fill advances one layer per step through free cells; a
    /// cell belongs to whoever reaches it in an earlier layer, and a cell both
    /// reach in the same layer goes to the longer serpent.
    #[must_use]
    pub fn partition(&self, board: &DuelBoard) -> Partition {
        let free = board.occupied().complement();
        let us = board.serpent(Side::Us);
        let them = board.serpent(Side::Them);
        let tie_winner = us.length().cmp(&them.length());

        let mut our_front = CellSet::single(us.head());
        let mut their_front = CellSet::single(them.head());
        let mut our_seen = our_front;
        let mut their_seen = their_front;
        let mut decided = CellSet::EMPTY;
        let mut partition = Partition {
            ours: CellSet::EMPTY,
            theirs: CellSet::EMPTY,
        };

        while !our_front.is_empty() || !their_front.is_empty() {
            our_front = expand_layer(our_front, free, our_seen);
            their_front = expand_layer(their_front, free, their_seen);
            our_seen = our_seen.union(our_front);
            their_seen = their_seen.union(their_front);

            let fresh_ours = our_front.difference(decided);
            let fresh_theirs = their_front.difference(decided);
            let tied = fresh_ours.intersection(fresh_theirs);
            partition.ours = partition.ours.union(fresh_ours.difference(tied));
            partition.theirs = partition.theirs.union(fresh_theirs.difference(tied));
            match tie_winner {
                Ordering::Greater => partition.ours = partition.ours.union(tied),
                Ordering::Less => partition.theirs = partition.theirs.union(tied),
                Ordering::Equal => {}
            }
            decided = decided.union(fresh_ours).union(fresh_theirs);
        }

        partition
    }
}

/// One fill step: the free, not-yet-seen neighbours of the frontier.
fn expand_layer(front: CellSet, free: CellSet, seen: CellSet) -> CellSet {
    front.neighbours().intersection(free).difference(seen)
}

impl Assessor for Dominion {
    const NAME: &'static str = "dominion";

    fn assess(&self, board: &DuelBoard) -> i32 {
        let partition = self.partition(board);
        partition.ours.len().cast_signed() - partition.theirs.len().cast_signed()
    }
}
