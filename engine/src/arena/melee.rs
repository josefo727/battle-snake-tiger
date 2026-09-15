//! A melee position: two to four serpents, the food on the board, and a ply
//! counter. Our serpent always sits in seat 0. Plain `Copy` data like the duel
//! board, so search copies positions instead of undoing moves.

use super::cellset::CellSet;
use super::duel::{BoardError, DuelBoard, check_invariants};
use super::serpent::Serpent;

/// The most serpents a melee holds.
pub const MAX_SEATS: usize = 4;

/// One of the four positions a serpent can occupy; seat 0 is ours.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Seat(u8);

impl Seat {
    pub const US: Self = Self(0);
    pub const ALL: [Self; MAX_SEATS] = [Self(0), Self(1), Self(2), Self(3)];

    #[must_use]
    pub const fn new(index: usize) -> Option<Self> {
        if index < MAX_SEATS {
            Some(Self(index as u8))
        } else {
            None
        }
    }

    #[must_use]
    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

#[derive(Clone, Copy, Debug)]
pub struct MeleeBoard {
    serpents: [Serpent; MAX_SEATS],
    /// Bit `i` set: seat `i` holds a living serpent.
    alive: u8,
    pellets: CellSet,
    ply: u16,
}

impl MeleeBoard {
    pub const MAX_VIGOR: u8 = DuelBoard::MAX_VIGOR;

    /// Builds a starting position, our serpent first, after checking its
    /// invariants.
    ///
    /// # Errors
    ///
    /// Rejects fewer than two or more than four serpents, serpents that share a
    /// cell, a pellet under a serpent, and a vigor outside `1..=100`.
    pub fn try_new(serpents: &[Serpent], pellets: CellSet) -> Result<Self, BoardError> {
        if !(2..=MAX_SEATS).contains(&serpents.len()) {
            return Err(BoardError::SeatCount);
        }
        check_invariants(serpents, pellets)?;
        // Unused seats hold a copy of our serpent; they are never alive, so never read.
        let mut seats = [serpents[0]; MAX_SEATS];
        seats[..serpents.len()].copy_from_slice(serpents);
        Ok(Self {
            serpents: seats,
            alive: (1u8 << serpents.len()) - 1,
            pellets,
            ply: 0,
        })
    }

    /// The living seats, in seat order.
    pub fn seats(&self) -> impl Iterator<Item = Seat> + '_ {
        Seat::ALL.into_iter().filter(|seat| self.is_alive(*seat))
    }

    #[must_use]
    pub const fn is_alive(&self, seat: Seat) -> bool {
        self.alive >> seat.0 & 1 == 1
    }

    #[must_use]
    pub const fn alive_count(&self) -> u8 {
        self.alive.count_ones() as u8
    }

    /// The serpent in `seat`, which must be alive.
    #[must_use]
    pub const fn serpent(&self, seat: Seat) -> &Serpent {
        &self.serpents[seat.index()]
    }

    #[must_use]
    pub const fn pellets(&self) -> CellSet {
        self.pellets
    }

    #[must_use]
    pub const fn ply(&self) -> u16 {
        self.ply
    }

    /// Every cell occupied by a living serpent.
    #[must_use]
    pub fn occupied(&self) -> CellSet {
        self.seats().fold(CellSet::EMPTY, |cells, seat| {
            cells.union(self.serpent(seat).cells())
        })
    }

    /// The duel view of a position where only we and one opponent are left.
    ///
    /// # Panics
    ///
    /// Panics unless exactly two seats are alive and one of them is ours.
    #[must_use]
    pub fn as_duel(&self) -> DuelBoard {
        assert!(
            self.alive_count() == 2 && self.is_alive(Seat::US),
            "a duel view needs us and exactly one opponent"
        );
        let them = self
            .seats()
            .find(|seat| *seat != Seat::US)
            .expect("two seats are alive");
        DuelBoard::try_new(self.serpents[0], self.serpents[them.index()], self.pellets)
            .expect("a valid melee position is a valid duel")
    }
}
