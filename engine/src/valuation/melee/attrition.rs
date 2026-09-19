//! Attrition: how many seats have already been eliminated.

use super::Surveyed;
use crate::arena::melee::MAX_SEATS;
use crate::valuation::Assessor;

/// One point per seat no longer alive, out of the four a melee can hold.
#[derive(Clone, Copy, Debug, Default)]
pub struct Attrition;

impl Assessor for Attrition {
    type Board = Surveyed;

    const NAME: &'static str = "attrition";
    const MAX_RAW: i32 = 3;

    fn assess(&self, position: &Surveyed) -> i32 {
        MAX_SEATS as i32 - i32::from(position.board.alive_count())
    }
}
