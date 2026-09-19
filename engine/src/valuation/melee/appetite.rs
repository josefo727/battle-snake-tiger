//! Appetite: the pull of the nearest pellet we reach before every rival, whatever
//! our health, so the engine goes for the food it wins the race to instead of
//! waiting to be hungry.

use super::territory::Territory;
use crate::arena::melee::{MeleeBoard, Seat};
use crate::valuation::Assessor;

/// Pellets farther than this (in turns) pull nothing.
pub const REACH: i32 = 12;

#[derive(Clone, Copy, Debug, Default)]
pub struct Appetite;

impl Assessor for Appetite {
    type Board = MeleeBoard;

    const NAME: &'static str = "appetite";
    const MAX_RAW: i32 = REACH;

    /// `REACH - distance` to the nearest pellet we own, at least zero; zero when
    /// we own no pellet.
    fn assess(&self, board: &MeleeBoard) -> i32 {
        let survey = Territory.survey(board);
        survey.food[Seat::US.index()].map_or(0, |distance| (REACH - i32::from(distance)).max(0))
    }
}
