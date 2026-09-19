//! Hunger: how close we are to starving before the nearest pellet we own.

use super::Surveyed;
use crate::arena::melee::Seat;
use crate::valuation::Assessor;
use crate::valuation::sustenance::{MAX_PRESSURE, pressure};

/// Minus our hunger pressure (the duel formula): zero while comfortable, up to
/// `MAX_PRESSURE` once food is out of reach.
#[derive(Clone, Copy, Debug, Default)]
pub struct Hunger;

impl Assessor for Hunger {
    type Board = Surveyed;

    const NAME: &'static str = "hunger";
    const MAX_RAW: i32 = MAX_PRESSURE;

    fn assess(&self, position: &Surveyed) -> i32 {
        -pressure(
            position.board.serpent(Seat::US).vigor(),
            position.survey.food[Seat::US.index()],
        )
    }
}
