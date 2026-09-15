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
    /// A melee holds two to four serpents.
    SeatCount,
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
        check_invariants(&[us, them], pellets)?;
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
        let reports = next.movement_phase(us, them);
        next.feeding_phase();
        match next.elimination_phase(&reports) {
            Some(verdict) => Advance::Over(verdict),
            None => Advance::Continues(next),
        }
    }

    /// Phase 1 of a turn: both serpents move from the same starting board.
    fn movement_phase(&mut self, us: Heading, them: Heading) -> [MoveReport; 2] {
        [
            move_serpent(&mut self.serpents[Side::Us.index()], us),
            move_serpent(&mut self.serpents[Side::Them.index()], them),
        ]
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

    /// Phase 3: eliminations are judged on the post-move snapshot of both
    /// serpents at once, so a serpent that dies this turn still blocks.
    fn elimination_phase(&self, reports: &[MoveReport; 2]) -> Option<Verdict> {
        match [Side::Us, Side::Them].map(|side| self.is_eliminated(side, reports)) {
            [false, false] => None,
            [false, true] => Some(Verdict::WeOnly),
            [true, false] => Some(Verdict::TheyOnly),
            [true, true] => Some(Verdict::BothDown),
        }
    }

    fn is_eliminated(&self, side: Side, reports: &[MoveReport; 2]) -> bool {
        let serpent = &self.serpents[side.index()];
        let own = &reports[side.index()];
        let opposing_segments = reports[side.other().index()].segments;

        serpent.vigor() == 0
            || own.off_board
            || own.self_hit
            || opposing_segments.contains(serpent.head())
            || self.loses_head_to_head(side, reports)
    }

    /// Two heads in one cell: only a strictly longer serpent survives, so equal
    /// lengths eliminate both. Lengths are post-move and post-feeding.
    fn loses_head_to_head(&self, side: Side, reports: &[MoveReport; 2]) -> bool {
        let rival_side = side.other();
        let own = &self.serpents[side.index()];
        let rival = &self.serpents[rival_side.index()];

        !reports[side.index()].off_board
            && !reports[rival_side.index()].off_board
            && own.head() == rival.head()
            && own.length() <= rival.length()
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

/// The invariants every starting position shares: each serpent's vigor in
/// `1..=MAX_VIGOR`, no two serpents sharing a cell, no pellet under a serpent.
pub(super) fn check_invariants(serpents: &[Serpent], pellets: CellSet) -> Result<(), BoardError> {
    let mut taken = CellSet::EMPTY;
    for serpent in serpents {
        if !(1..=DuelBoard::MAX_VIGOR).contains(&serpent.vigor()) {
            return Err(BoardError::VigorOutOfRange);
        }
        if !taken.intersection(serpent.cells()).is_empty() {
            return Err(BoardError::SerpentsOverlap);
        }
        taken = taken.union(serpent.cells());
    }
    if !pellets.intersection(taken).is_empty() {
        return Err(BoardError::PelletUnderSerpent);
    }
    Ok(())
}

/// What one serpent's move leaves behind for the elimination phase.
struct MoveReport {
    /// The step left the board (the serpent is left unmoved and is eliminated).
    off_board: bool,
    /// The new head landed on a cell the serpent's own body still holds.
    self_hit: bool,
    /// Cells of every segment after the head, as the other serpent sees them.
    segments: CellSet,
}

/// Movement phase for one serpent: new head, old tail released, one vigor lost.
fn move_serpent(serpent: &mut Serpent, heading: Heading) -> MoveReport {
    serpent.lose_vigor();
    let Some(target) = heading.step(serpent.head()) else {
        return MoveReport {
            off_board: true,
            self_hit: false,
            segments: segments_without_tail(serpent),
        };
    };

    let occupied_before = serpent.cells().contains(target);
    serpent.advance_head(target);
    let released = serpent.release_tail();
    // Entering the cell the tail just left is safe unless a stacked copy remains.
    let self_hit = occupied_before && (target != released || serpent.tail() == released);
    let segments = if self_hit {
        serpent.cells()
    } else {
        serpent.cells().without(target)
    };

    MoveReport {
        off_board: false,
        self_hit,
        segments,
    }
}

/// A serpent that leaves the board keeps its old head as a body segment and
/// still drops its tail, exactly as the reused resolver reports it.
fn segments_without_tail(serpent: &Serpent) -> CellSet {
    if serpent.length() == 1 {
        return CellSet::EMPTY;
    }
    let mut ghost = *serpent;
    ghost.release_tail();
    ghost.cells()
}
