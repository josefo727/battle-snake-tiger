//! Endgame in a melee: when a seat is walled off from every other, what it has
//! left is not ground to contest but turns of life to spend (growth iteration 18).
//!
//! The duel has carried this term since the beginning, and the melee — where
//! three deaths in four are enclosure — has not. The question is the same one,
//! asked of a board with more seats on it: the room a serpent can reach on its
//! own, bounded by the colours a path through it must alternate and by the
//! health it has to walk them.

use super::Surveyed;
use crate::arena::cellset::CellSet;
use crate::arena::melee::{MeleeBoard, Seat};
use crate::valuation::Assessor;
use crate::valuation::enclosure::{own_room, static_region, survival_in};

#[derive(Clone, Copy, Debug, Default)]
pub struct MeleeEnclosure;

impl MeleeEnclosure {
    /// The most turns `seat` can survive in the room it can reach alone, every
    /// other body a wall; zero for a seat that is no longer playing.
    #[must_use]
    pub fn survival_estimate(&self, board: &MeleeBoard, seat: Seat) -> i32 {
        if !board.is_alive(seat) {
            return 0;
        }
        let serpent = board.serpent(seat);
        survival_in(
            own_room(board.occupied(), serpent),
            serpent,
            board.pellets(),
        )
    }
}

impl Assessor for MeleeEnclosure {
    type Board = Surveyed;

    const NAME: &'static str = "melee_enclosure";
    const MAX_RAW: i32 = 121;

    /// Our turns of life against the best any rival has, but only once we are
    /// shut away from all of them; while anyone can still walk onto our ground
    /// the position is a contest over territory and the term keeps quiet.
    ///
    /// The cage that ends these games is built by two rivals and our own body
    /// at once, so the gate asks about every living seat rather than the one
    /// that happens to be nearest.
    ///
    /// The gate costs one fill, not one per seat. Free ground splits into
    /// connected pieces and a serpent's own is the piece its head touches, so
    /// two serpents share ground exactly when a free neighbour of the rival's
    /// head lies in ours: the pieces are the same one or they are disjoint, and
    /// there is no third case to check.
    fn assess(&self, position: &Surveyed) -> i32 {
        let board = &position.board;
        let occupied = board.occupied();
        let ours = static_region(occupied, board.serpent(Seat::US).head());
        let rivals = || board.seats().filter(|seat| *seat != Seat::US);
        if rivals().any(|seat| reaches(ours, board, seat)) {
            return 0;
        }

        let best_rival = rivals()
            .map(|seat| self.survival_estimate(board, seat))
            .max()
            .unwrap_or(0);
        self.survival_estimate(board, Seat::US) - best_rival
    }
}

/// Whether `seat` can step onto `ours` from where its head stands.
fn reaches(ours: CellSet, board: &MeleeBoard, seat: Seat) -> bool {
    !CellSet::single(board.serpent(seat).head())
        .neighbours()
        .intersection(ours)
        .is_empty()
}
