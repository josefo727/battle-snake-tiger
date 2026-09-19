//! Larder: the pellets we reach before every rival, each a meal only we can
//! take (growth iteration 6).

use super::Surveyed;
use crate::arena::melee::Seat;
use crate::valuation::Assessor;

#[derive(Clone, Copy, Debug, Default)]
pub struct Larder;

impl Assessor for Larder {
    type Board = Surveyed;

    const NAME: &'static str = "larder";
    const MAX_RAW: i32 = 121;

    /// How many pellets lie in the cells we own.
    fn assess(&self, position: &Surveyed) -> i32 {
        position.survey.owned[Seat::US.index()]
            .intersection(position.board.pellets())
            .len()
            .cast_signed()
    }
}
