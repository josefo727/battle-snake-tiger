//! Larder: the pellets we reach before every rival, each a meal only we can
//! take (growth iteration 6).

use super::territory::Territory;
use crate::arena::melee::{MeleeBoard, Seat};
use crate::valuation::Assessor;

#[derive(Clone, Copy, Debug, Default)]
pub struct Larder;

impl Assessor for Larder {
    type Board = MeleeBoard;

    const NAME: &'static str = "larder";
    const MAX_RAW: i32 = 121;

    /// How many pellets lie in the cells we own.
    fn assess(&self, board: &MeleeBoard) -> i32 {
        let survey = Territory.survey(board);
        survey.owned[Seat::US.index()]
            .intersection(board.pellets())
            .len()
            .cast_signed()
    }
}
