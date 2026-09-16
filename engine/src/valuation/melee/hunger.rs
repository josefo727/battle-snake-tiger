//! Hunger: how close we are to starving before the nearest pellet we own.

use super::territory::Territory;
use crate::arena::melee::{MeleeBoard, Seat};
use crate::valuation::Assessor;
use crate::valuation::sustenance::{MAX_PRESSURE, pressure};

/// Minus our hunger pressure (the duel formula): zero while comfortable, up to
/// `MAX_PRESSURE` once food is out of reach.
#[derive(Clone, Copy, Debug, Default)]
pub struct Hunger;

impl Assessor for Hunger {
    type Board = MeleeBoard;

    const NAME: &'static str = "hunger";
    const MAX_RAW: i32 = MAX_PRESSURE;

    fn assess(&self, board: &MeleeBoard) -> i32 {
        let survey = Territory.survey(board);
        -pressure(
            board.serpent(Seat::US).vigor(),
            survey.food[Seat::US.index()],
        )
    }
}
