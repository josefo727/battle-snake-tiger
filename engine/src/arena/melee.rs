//! A melee position: two to four serpents, the food on the board, and a ply
//! counter. Our serpent always sits in seat 0. Plain `Copy` data like the duel
//! board, so search copies positions instead of undoing moves.

use super::cellset::CellSet;
use super::duel::{BoardError, DuelBoard, MoveReport, check_invariants, move_serpent};
use super::heading::Heading;
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

/// The result of resolving one joint move of a melee.
// Positions are copied by value on purpose (no allocation on the search hot
// path), so the size gap between the variants is accepted.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Copy, Debug)]
pub enum MeleeOutcome {
    /// We and at least one opponent are still alive.
    Continues(MeleeBoard),
    /// We are the last serpent standing.
    WeAlone,
    /// We were eliminated with `rivals_left` opponents still alive (zero when
    /// every serpent died in the same turn).
    WeDown { rivals_left: u8 },
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

    /// Resolves one simultaneous turn: every living seat moves by its heading
    /// (dead seats' headings are ignored).
    #[must_use]
    pub fn advance(&self, moves: &[Heading; MAX_SEATS]) -> MeleeOutcome {
        let mut next = *self;
        next.ply += 1;
        let reports = next.movement_phase(moves);
        next.feeding_phase();
        next.elimination_phase(&reports);
        match (next.is_alive(Seat::US), next.alive_count()) {
            (true, 1) => MeleeOutcome::WeAlone,
            (true, _) => MeleeOutcome::Continues(next),
            (false, rivals_left) => MeleeOutcome::WeDown { rivals_left },
        }
    }

    /// Phase 3: eliminations are judged on the post-move snapshot of every
    /// living seat at once, so a seat that dies this turn still blocks and
    /// still takes part in a head-to-head.
    fn elimination_phase(&mut self, reports: &[MoveReport; MAX_SEATS]) {
        let mut survivors = self.alive;
        for seat in self.seats() {
            if self.is_eliminated(seat, reports) {
                survivors &= !(1 << seat.0);
            }
        }
        self.alive = survivors;
    }

    fn is_eliminated(&self, seat: Seat, reports: &[MoveReport; MAX_SEATS]) -> bool {
        let serpent = self.serpent(seat);
        let own = &reports[seat.index()];
        serpent.vigor() == 0
            || own.off_board
            || own.self_hit
            || self
                .seats()
                .filter(|other| *other != seat)
                .any(|other| reports[other.index()].segments.contains(serpent.head()))
            || self.loses_head_to_head(seat, reports)
    }

    /// Several heads in one cell: only a strictly longest serpent survives, so a
    /// seat loses when any other head there is at least as long. Lengths are
    /// post-move and post-feeding; a seat that left the board has no head there.
    fn loses_head_to_head(&self, seat: Seat, reports: &[MoveReport; MAX_SEATS]) -> bool {
        if reports[seat.index()].off_board {
            return false;
        }
        let own = self.serpent(seat);
        self.seats()
            .filter(|other| *other != seat && !reports[other.index()].off_board)
            .map(|other| self.serpent(other))
            .any(|rival| rival.head() == own.head() && rival.length() >= own.length())
    }

    /// Phase 1 of a turn: every living seat moves from the same starting board.
    fn movement_phase(&mut self, moves: &[Heading; MAX_SEATS]) -> [MoveReport; MAX_SEATS] {
        let mut reports = [MoveReport::IDLE; MAX_SEATS];
        for seat in Seat::ALL {
            if self.is_alive(seat) {
                reports[seat.index()] =
                    move_serpent(&mut self.serpents[seat.index()], moves[seat.index()]);
            }
        }
        reports
    }

    /// Phase 2: any living head on a pellet eats; every eaten pellet is removed
    /// afterwards so several heads entering one cell all eat.
    fn feeding_phase(&mut self) {
        let mut eaten = CellSet::EMPTY;
        for seat in Seat::ALL {
            if !self.is_alive(seat) {
                continue;
            }
            let serpent = &mut self.serpents[seat.index()];
            let head = serpent.head();
            if self.pellets.contains(head) {
                serpent.eat(Self::MAX_VIGOR);
                eaten = eaten.with(head);
            }
        }
        self.pellets = self.pellets.difference(eaten);
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
