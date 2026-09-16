//! Leverage: what being longer is worth, in raw length and at a head-to-head.

use core::cmp::Ordering;

use super::Assessor;
use crate::arena::cellset::CellSet;
use crate::arena::duel::{DuelBoard, Side};

/// Our length minus theirs, in segments.
#[derive(Clone, Copy, Debug, Default)]
pub struct LengthAdvantage;

impl Assessor for LengthAdvantage {
    type Board = DuelBoard;

    const NAME: &'static str = "length_advantage";
    const MAX_RAW: i32 = 120;

    fn assess(&self, board: &DuelBoard) -> i32 {
        i32::from(board.serpent(Side::Us).length()) - i32::from(board.serpent(Side::Them).length())
    }
}

/// How many cells both heads can enter next turn, signed by who would win the
/// head-to-head there: positive when we are longer, negative when they are,
/// zero at equal length (a tie kills both).
#[derive(Clone, Copy, Debug, Default)]
pub struct HeadPressure;

impl Assessor for HeadPressure {
    type Board = DuelBoard;

    const NAME: &'static str = "head_pressure";
    const MAX_RAW: i32 = 4;

    fn assess(&self, board: &DuelBoard) -> i32 {
        let us = board.serpent(Side::Us);
        let them = board.serpent(Side::Them);

        // Free cells plus the tail cells that vacate this turn.
        let mut enterable = board.occupied().complement();
        for serpent in [us, them] {
            if let Some(cell) = serpent.cell_released_on_turn(1) {
                enterable = enterable.with(cell);
            }
        }
        let reach = |head| CellSet::single(head).neighbours().intersection(enterable);
        let contested = reach(us.head())
            .intersection(reach(them.head()))
            .len()
            .cast_signed();

        match us.length().cmp(&them.length()) {
            Ordering::Greater => contested,
            Ordering::Less => -contested,
            Ordering::Equal => 0,
        }
    }
}
