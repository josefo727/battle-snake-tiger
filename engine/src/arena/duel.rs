//! A duel position: exactly two serpents, the food on the board, and a ply
//! counter. Plain `Copy` data so search can copy positions instead of undoing.

use super::cellset::CellSet;
use super::heading::Heading;
use super::serpent::Serpent;

/// Which serpent: the one we control, or the opponent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Us,
    Them,
}

impl Side {
    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Self::Us => 0,
            Self::Them => 1,
        }
    }

    #[must_use]
    pub const fn other(self) -> Self {
        match self {
            Self::Us => Self::Them,
            Self::Them => Self::Us,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoardError {
    SerpentsOverlap,
    PelletUnderSerpent,
    VigorOutOfRange,
}

/// Who is left standing when a duel ends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    WeOnly,
    TheyOnly,
    BothDown,
}

/// The result of resolving one joint move.
// Positions are copied by value on purpose (no allocation on the search hot
// path), so the size gap between the variants is accepted.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Copy, Debug)]
pub enum Advance {
    Continues(DuelBoard),
    Over(Verdict),
}

#[derive(Clone, Copy, Debug)]
pub struct DuelBoard {
    serpents: [Serpent; 2],
    pellets: CellSet,
    ply: u16,
}

impl DuelBoard {
    pub const MAX_VIGOR: u8 = 100;

    /// Builds a starting position after checking its invariants.
    ///
    /// # Errors
    ///
    /// Rejects serpents that share a cell, a pellet under a serpent, and a
    /// vigor outside `1..=100`.
    pub fn try_new(us: Serpent, them: Serpent, pellets: CellSet) -> Result<Self, BoardError> {
        for serpent in [&us, &them] {
            if !(1..=Self::MAX_VIGOR).contains(&serpent.vigor()) {
                return Err(BoardError::VigorOutOfRange);
            }
        }
        if !us.cells().intersection(them.cells()).is_empty() {
            return Err(BoardError::SerpentsOverlap);
        }
        if !pellets
            .intersection(us.cells().union(them.cells()))
            .is_empty()
        {
            return Err(BoardError::PelletUnderSerpent);
        }

        Ok(Self {
            serpents: [us, them],
            pellets,
            ply: 0,
        })
    }

    /// Resolves one simultaneous turn from this immutable position.
    #[must_use]
    pub fn advance(&self, us: Heading, them: Heading) -> Advance {
        let mut next = *self;
        next.ply += 1;
        next.movement_phase(us, them);
        next.feeding_phase();
        Advance::Continues(next)
    }

    /// Phase 1 of a turn: both serpents move from the same starting board.
    fn movement_phase(&mut self, us: Heading, them: Heading) {
        move_serpent(&mut self.serpents[Side::Us.index()], us);
        move_serpent(&mut self.serpents[Side::Them.index()], them);
    }

    /// Phase 2: any serpent on a pellet eats; every eaten pellet is removed
    /// afterwards so two serpents entering one cell both eat.
    fn feeding_phase(&mut self) {
        let mut eaten = CellSet::EMPTY;
        for serpent in &mut self.serpents {
            let head = serpent.head();
            if self.pellets.contains(head) {
                serpent.eat(Self::MAX_VIGOR);
                eaten = eaten.with(head);
            }
        }
        self.pellets = self.pellets.difference(eaten);
    }

    #[must_use]
    pub const fn serpent(&self, side: Side) -> &Serpent {
        &self.serpents[side.index()]
    }

    #[must_use]
    pub const fn pellets(&self) -> CellSet {
        self.pellets
    }

    #[must_use]
    pub const fn ply(&self) -> u16 {
        self.ply
    }

    /// Every cell occupied by either serpent.
    #[must_use]
    pub const fn occupied(&self) -> CellSet {
        self.serpents[0].cells().union(self.serpents[1].cells())
    }
}

/// Movement phase for one serpent: new head, old tail released, one vigor
/// lost. A step off the board leaves the serpent unmoved here; eliminating it
/// belongs to the elimination phase.
fn move_serpent(serpent: &mut Serpent, heading: Heading) {
    if let Some(target) = heading.step(serpent.head()) {
        serpent.advance_head(target);
        serpent.release_tail();
    }
    serpent.lose_vigor();
}
