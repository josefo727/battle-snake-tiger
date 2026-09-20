//! The finisher (growth iteration 10): when we are strictly longer than a rival
//! whose head is near ours, a position is better the fewer exits that head has
//! and the less territory it owns, so the search closes on it instead of
//! keeping its distance.

use super::Surveyed;
use crate::arena::cellset::CellSet;
use crate::arena::melee::Seat;
use crate::valuation::Assessor;

/// Heads at most this many steps apart are in contact.
pub const CONTACT: u32 = 4;
/// A rival owning this many cells or fewer is being squeezed.
pub const CRAMPED: i32 = 8;

#[derive(Clone, Copy, Debug, Default)]
pub struct Finisher;

impl Assessor for Finisher {
    type Board = Surveyed;

    const NAME: &'static str = "finisher";
    /// Three rivals, each at most `3 + CRAMPED`.
    const MAX_RAW: i32 = 3 * (3 + CRAMPED);

    fn assess(&self, position: &Surveyed) -> i32 {
        let board = &position.board;
        let us = board.serpent(Seat::US);
        let free = board.occupied().complement();
        board
            .seats()
            .filter(|seat| *seat != Seat::US)
            .filter(|seat| {
                let rival = board.serpent(*seat);
                rival.length() < us.length() && distance(rival.head(), us.head()) <= CONTACT
            })
            .map(|seat| {
                let rival = board.serpent(seat);
                let exits = CellSet::single(rival.head())
                    .neighbours()
                    .intersection(free)
                    .len()
                    .cast_signed();
                let owned = position.survey.owned[seat.index()].len().cast_signed();
                (3 - exits.min(3)) + (CRAMPED - owned).max(0)
            })
            .sum()
    }
}

fn distance(a: crate::arena::cellset::Cell, b: crate::arena::cellset::Cell) -> u32 {
    u32::from(a.x().abs_diff(b.x()) + a.y().abs_diff(b.y()))
}
