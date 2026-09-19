//! Territory among several serpents: which cells each seat reaches strictly
//! before every other, as bodies move away.

use super::Surveyed;
use crate::arena::cellset::CellSet;
use crate::arena::melee::{MAX_SEATS, MeleeBoard, Seat};
use crate::arena::serpent::Serpent;
use crate::valuation::Assessor;
use crate::valuation::fill::{Fill, MAX_LAYERS};

/// The cells each seat owns and how many turns it needs to reach the nearest
/// pellet it owns (`None` when it owns no pellet). Dead seats own nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MeleeSurvey {
    pub owned: [CellSet; MAX_SEATS],
    pub food: [Option<u16>; MAX_SEATS],
}

/// Our territory against the largest opponent's.
#[derive(Clone, Copy, Debug, Default)]
pub struct Territory;

impl Territory {
    /// Splits the cells between the living seats, modelling that bodies move away.
    ///
    /// Every seat's fill advances one layer per turn. A body cell is passable
    /// from the turn its last segment leaves it (the segment `i` places from the
    /// tail frees after `i + 1` turns; a stacked copy delays it one turn each).
    /// A cell belongs to whoever arrives first; on a same-turn arrival to the
    /// strictly longest of those seats, or to nobody. Head cells are never
    /// territory.
    #[must_use]
    pub fn survey(&self, board: &MeleeBoard) -> MeleeSurvey {
        let seats: [bool; MAX_SEATS] = Seat::ALL.map(|seat| board.is_alive(seat));
        let lengths: [u8; MAX_SEATS] = Seat::ALL.map(|seat| {
            if seats[seat.index()] {
                board.serpent(seat).length()
            } else {
                0
            }
        });
        let last_release = u16::from(lengths.iter().copied().max().unwrap_or(0));

        let mut free = board.occupied().complement();
        let mut fills: [Fill; MAX_SEATS] = Seat::ALL.map(|seat| {
            if seats[seat.index()] {
                Fill::from(board.serpent(seat).head())
            } else {
                Fill::exhausted()
            }
        });
        let mut decided = fills
            .iter()
            .fold(CellSet::EMPTY, |all, fill| all.union(fill.seen()));
        let mut survey = MeleeSurvey {
            owned: [CellSet::EMPTY; MAX_SEATS],
            food: [None; MAX_SEATS],
        };

        for layer in 1..=MAX_LAYERS {
            let released = board.seats().fold(CellSet::EMPTY, |all, seat| {
                all.union(released_on_turn(board.serpent(seat), layer))
            });
            free = free.union(released);
            let fresh: [CellSet; MAX_SEATS] = fills
                .each_mut()
                .map(|fill| fill.advance(free, released).difference(decided));

            for seat in Seat::ALL {
                let index = seat.index();
                if !seats[index] {
                    continue;
                }
                // A cell reached this turn by a seat at least as long is lost.
                let contested = (0..MAX_SEATS)
                    .filter(|other| {
                        *other != index && seats[*other] && lengths[*other] >= lengths[index]
                    })
                    .fold(CellSet::EMPTY, |all, other| all.union(fresh[other]));
                let won = fresh[index].difference(contested);
                survey.owned[index] = survey.owned[index].union(won);
                if survey.food[index].is_none() && !won.intersection(board.pellets()).is_empty() {
                    survey.food[index] = Some(layer);
                }
            }
            decided = fresh.iter().fold(decided, |all, cells| all.union(*cells));

            if layer >= last_release && fills.iter().all(Fill::is_exhausted) {
                break;
            }
        }
        survey
    }
}

impl Assessor for Territory {
    type Board = Surveyed;

    const NAME: &'static str = "territory";
    const MAX_RAW: i32 = 121;

    /// Our cells minus the most cells any living opponent owns.
    fn assess(&self, position: &Surveyed) -> i32 {
        let survey = &position.survey;
        let ours = survey.owned[Seat::US.index()].len().cast_signed();
        let best_rival = position
            .board
            .seats()
            .filter(|seat| *seat != Seat::US)
            .map(|seat| survey.owned[seat.index()].len().cast_signed())
            .max()
            .unwrap_or(0);
        ours - best_rival
    }
}

/// The cells `serpent`'s body vacates entirely on turn `layer`, as a set.
fn released_on_turn(serpent: &Serpent, layer: u16) -> CellSet {
    serpent
        .cell_released_on_turn(layer)
        .map_or(CellSet::EMPTY, CellSet::single)
}
