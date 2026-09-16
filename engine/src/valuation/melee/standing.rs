//! Standing among several serpents: our length against the longest rival, and
//! the head-to-head threats and chances on the cells we can enter next.

use crate::arena::cellset::CellSet;
use crate::arena::melee::{MeleeBoard, Seat};
use crate::valuation::Assessor;

/// Our length minus the longest living opponent's, in segments.
#[derive(Clone, Copy, Debug, Default)]
pub struct Standing;

impl Assessor for Standing {
    type Board = MeleeBoard;

    const NAME: &'static str = "standing";
    const MAX_RAW: i32 = 120;

    fn assess(&self, board: &MeleeBoard) -> i32 {
        let longest_rival = board
            .seats()
            .filter(|seat| *seat != Seat::US)
            .map(|seat| board.serpent(seat).length())
            .max()
            .unwrap_or(0);
        i32::from(board.serpent(Seat::US).length()) - i32::from(longest_rival)
    }
}

/// Over the cells we can enter next turn: minus one for each cell an equal or
/// longer opponent head can also enter (a head-to-head we cannot win), plus one
/// for each cell only shorter heads can enter (one we would win).
#[derive(Clone, Copy, Debug, Default)]
pub struct HeadDanger;

impl Assessor for HeadDanger {
    type Board = MeleeBoard;

    const NAME: &'static str = "head_danger";
    const MAX_RAW: i32 = 4;

    fn assess(&self, board: &MeleeBoard) -> i32 {
        let enterable = enterable_cells(board);
        let reach = |seat: Seat| {
            CellSet::single(board.serpent(seat).head())
                .neighbours()
                .intersection(enterable)
        };
        let ours = reach(Seat::US);
        let our_length = board.serpent(Seat::US).length();
        let mut dangerous = CellSet::EMPTY;
        let mut winnable = CellSet::EMPTY;
        for seat in board.seats().filter(|seat| *seat != Seat::US) {
            let shared = reach(seat).intersection(ours);
            if board.serpent(seat).length() >= our_length {
                dangerous = dangerous.union(shared);
            } else {
                winnable = winnable.union(shared);
            }
        }
        let winnable = winnable.difference(dangerous);
        winnable.len().cast_signed() - dangerous.len().cast_signed()
    }
}

/// Free cells plus the tail cells that vacate this turn.
fn enterable_cells(board: &MeleeBoard) -> CellSet {
    board
        .seats()
        .fold(board.occupied().complement(), |cells, seat| {
            board
                .serpent(seat)
                .cell_released_on_turn(1)
                .map_or(cells, |cell| cells.with(cell))
        })
}
