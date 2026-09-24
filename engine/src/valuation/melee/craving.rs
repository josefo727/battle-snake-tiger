//! Craving: the pull of the nearest pellet we can reach at all, whether or not
//! we would get there first (growth iteration 19).
//!
//! [`super::appetite::Appetite`], [`super::larder::Larder`] and
//! [`super::hunger::Hunger`] all read the pellets the territory survey grants
//! us, which are the ones we reach strictly before every rival. Measured over
//! 302 positions of ordinary play on 2026-09-24, all three are exactly zero in
//! 48% of them: half the game with no pull toward food of any kind, while we
//! ate about a quarter fewer pellets than every rival in every window from turn
//! zero and died shorter than the longest rival in 34 of 35 games.
//!
//! Ownership was not the asymmetry -- we held 0.60 pellets a position against
//! the rivals' 0.61 a seat. The asymmetry was that a pellet we do not own pulls
//! nothing, so a contested pellet is one we simply do not go for. Shapeshifter
//! and snork both carry a plain distance-to-food term that never goes quiet;
//! this is ours.

use super::Surveyed;
use super::appetite::REACH;
use crate::arena::cellset::CellSet;
use crate::arena::melee::{MeleeBoard, Seat};
use crate::arena::serpent::Serpent;
use crate::valuation::Assessor;
use crate::valuation::fill::{Fill, MAX_LAYERS};

#[derive(Clone, Copy, Debug, Default)]
pub struct Craving;

impl Assessor for Craving {
    type Board = Surveyed;

    const NAME: &'static str = "craving";
    const MAX_RAW: i32 = REACH;

    /// `REACH - distance` to the nearest pellet we can reach, at least zero;
    /// zero when none is reachable at all.
    fn assess(&self, position: &Surveyed) -> i32 {
        turns_to_food(&position.board, Seat::US)
            .map_or(0, |distance| (REACH - i32::from(distance)).max(0))
    }
}

/// Turns until `seat` could first stand on a pellet: its own body frees as it
/// goes and every other body is a wall, so this asks what the rules allow the
/// serpent to reach, not what it would win a race to.
///
/// `None` when no pellet is reachable, or when the nearest is further than
/// [`REACH`], which the term would price at nothing anyway.
#[must_use]
pub fn turns_to_food(board: &MeleeBoard, seat: Seat) -> Option<u16> {
    if !board.is_alive(seat) {
        return None;
    }
    let serpent = board.serpent(seat);
    let pellets = board.pellets();
    if pellets.is_empty() {
        return None;
    }
    let horizon = MAX_LAYERS.min(u16::try_from(REACH).unwrap_or(MAX_LAYERS));
    reach(board.occupied(), serpent, pellets, horizon)
}

/// The first layer of `serpent`'s own fill that touches `pellets`.
fn reach(occupied: CellSet, serpent: &Serpent, pellets: CellSet, horizon: u16) -> Option<u16> {
    let mut free = occupied.complement();
    let mut fill = Fill::from(serpent.head());
    for layer in 1..=horizon {
        let released = serpent
            .cell_released_on_turn(layer)
            .map_or(CellSet::EMPTY, CellSet::single);
        free = free.union(released);
        let fresh = fill.advance(free, released);
        if !fresh.intersection(pellets).is_empty() {
            return Some(layer);
        }
        // An exhausted fill is not a finished one: a serpent shut in by its own
        // body waits for the tail to free the next cell and goes on from there,
        // so the walk only ends once the whole body has passed.
        if layer >= u16::from(serpent.length()) && fill.is_exhausted() {
            return None;
        }
    }
    None
}
