//! A duel position: exactly two serpents, the food on the board, and a ply
//! counter. Plain `Copy` data so search can copy positions instead of undoing.

use super::cellset::CellSet;
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
