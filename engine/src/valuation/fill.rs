//! The layered fill shared by territory and enclosure: how far one serpent can
//! reach, turn by turn, as bodies move away.

use crate::arena::cellset::{Cell, CellSet};

/// Fill steps to run before giving up. A cell frees by turn 121 at the latest
/// and a path crosses at most 120 cells afterwards, so 250 always suffices.
pub const MAX_LAYERS: u16 = 250;

/// One serpent's fill: the cells reached on the latest turn and every cell
/// reached so far.
#[derive(Clone, Copy, Debug)]
pub struct Fill {
    front: CellSet,
    seen: CellSet,
}

impl Fill {
    pub fn from(head: Cell) -> Self {
        let start = CellSet::single(head);
        Self {
            front: start,
            seen: start,
        }
    }

    /// One turn: step into free unseen neighbours, and enter cells that freed
    /// this turn beside ground already reached (the serpent can dawdle there
    /// until they free). Returns the cells newly reached.
    pub fn advance(&mut self, free: CellSet, released: CellSet) -> CellSet {
        let by_walking = self.front.neighbours();
        let by_waiting = released.intersection(self.seen.neighbours());
        self.front = by_walking
            .union(by_waiting)
            .intersection(free)
            .difference(self.seen);
        self.seen = self.seen.union(self.front);
        self.front
    }

    pub const fn is_exhausted(&self) -> bool {
        self.front.is_empty()
    }

    /// Every cell reached so far, the start cell included.
    pub const fn seen(&self) -> CellSet {
        self.seen
    }
}
