//! Attrition: how many seats have already been eliminated.

use crate::arena::melee::{MAX_SEATS, MeleeBoard};
use crate::valuation::Assessor;

/// One point per seat no longer alive, out of the four a melee can hold.
#[derive(Clone, Copy, Debug, Default)]
pub struct Attrition;

impl Assessor for Attrition {
    type Board = MeleeBoard;

    const NAME: &'static str = "attrition";
    const MAX_RAW: i32 = 3;

    fn assess(&self, board: &MeleeBoard) -> i32 {
        MAX_SEATS as i32 - i32::from(board.alive_count())
    }
}
