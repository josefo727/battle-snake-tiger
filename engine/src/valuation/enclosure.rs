//! Endgame: when the serpents are walled apart, each one's survival is bounded
//! by the room it has left, so the contribution is our estimate minus theirs.

use super::Assessor;
use super::fill::{Fill, MAX_LAYERS};
use crate::arena::cellset::{Cell, CellSet};
use crate::arena::duel::{DuelBoard, Side};
use crate::arena::serpent::Serpent;

/// Turns of life a pellet restores at most (a pellet resets health to 100).
const PELLET_LIFE: i32 = 100;

#[derive(Clone, Copy, Debug, Default)]
pub struct Enclosure;

impl Enclosure {
    /// The most turns `side` can survive in the room it can reach, or zero when
    /// it has none.
    ///
    /// The room is what the serpent can reach through unoccupied cells plus its
    /// own body cells as they free (the other serpent's body counts as a wall);
    /// the checkerboard bound limits the path through it, and health plus
    /// [`PELLET_LIFE`] per pellet in the room caps how long it can keep going.
    #[must_use]
    pub fn survival_estimate(&self, board: &DuelBoard, side: Side) -> i32 {
        let serpent = board.serpent(side);
        survival_in(
            own_room(board.occupied(), serpent),
            serpent,
            board.pellets(),
        )
    }
}

/// The turns of life `room` is worth to `serpent`: the checkerboard bound on
/// the path through it, capped by the health it has and the [`PELLET_LIFE`]
/// each pellet in there restores.
///
/// Shared with the melee assessor, which asks the same question of a board with
/// three or four seats on it.
#[must_use]
pub fn survival_in(room: CellSet, serpent: &Serpent, pellets: CellSet) -> i32 {
    let in_reach = room.intersection(pellets).len().cast_signed();
    let starvation_cap = i32::from(serpent.vigor()) + PELLET_LIFE * in_reach;

    parity_bound(room, serpent.head()).min(starvation_cap)
}

/// Cells `serpent` can reach alone from an `occupied` board: free cells plus
/// its own body cells as they free, every other body a wall.
#[must_use]
pub fn own_room(occupied: CellSet, serpent: &Serpent) -> CellSet {
    let mut free = occupied.complement();
    let mut fill = Fill::from(serpent.head());
    for layer in 1..=MAX_LAYERS {
        let released = serpent
            .cell_released_on_turn(layer)
            .map_or(CellSet::EMPTY, CellSet::single);
        free = free.union(released);
        fill.advance(free, released);
        if layer >= u16::from(serpent.length()) && fill.is_exhausted() {
            break;
        }
    }
    fill.seen().without(serpent.head())
}

/// Cells reachable from `head` through the cells that are free right now.
#[must_use]
pub fn static_region(occupied: CellSet, head: Cell) -> CellSet {
    let free = occupied.complement();
    let mut seen = CellSet::EMPTY;
    let mut front = CellSet::single(head);
    loop {
        let next = front.neighbours().intersection(free).difference(seen);
        if next.is_empty() {
            return seen;
        }
        seen = seen.union(next);
        front = next;
    }
}

/// The longest path a serpent leaving `head` could take through `region`, using
/// the checkerboard bound: a path alternates colours, so it cannot use more cells
/// of one colour than the other allows.
#[must_use]
pub fn parity_bound(region: CellSet, head: Cell) -> i32 {
    let mut other = 0;
    let mut same = 0;
    for cell in region {
        if colour(cell) == colour(head) {
            same += 1;
        } else {
            other += 1;
        }
    }

    if other > same {
        2 * same + 1
    } else {
        2 * other
    }
}

const fn colour(cell: Cell) -> u8 {
    (cell.x() + cell.y()) & 1
}

impl Assessor for Enclosure {
    type Board = DuelBoard;

    const NAME: &'static str = "enclosure";
    const MAX_RAW: i32 = 121;

    /// Our survival estimate minus theirs, but only when the serpents are walled
    /// apart; while either can reach the other's ground the term is zero.
    fn assess(&self, board: &DuelBoard) -> i32 {
        let ours = static_region(board.occupied(), board.serpent(Side::Us).head());
        let theirs = static_region(board.occupied(), board.serpent(Side::Them).head());
        if !ours.intersection(theirs).is_empty() {
            return 0;
        }

        self.survival_estimate(board, Side::Us) - self.survival_estimate(board, Side::Them)
    }
}
