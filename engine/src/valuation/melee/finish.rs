//! Terminal scoring of a melee by placement: being the last alive is a win,
//! dying is worse the earlier it happens and the more rivals outlive us.

use super::weights::MeleeWeights;
use crate::arena::melee::{MAX_SEATS, MeleeOutcome};
use crate::valuation::finish::MAX_PLY;

/// The most rivals that can outlive us.
const MAX_RIVALS: i32 = MAX_SEATS as i32 - 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MeleeFinish {
    win_score: i32,
    draw_score: i32,
    ply_penalty: i32,
    placement_step: i32,
}

impl MeleeFinish {
    /// # Panics
    ///
    /// Panics when the weights would let a win at `MAX_PLY`, or a loss behind
    /// every rival at `MAX_PLY`, cross zero, which would invert the ordering
    /// search relies on.
    #[must_use]
    pub const fn new(weights: &MeleeWeights) -> Self {
        assert!(
            weights.win_score
                > weights.ply_penalty * MAX_PLY as i32 + weights.placement_step * MAX_RIVALS,
            "a forced win at MAX_PLY must still outscore any loss or position"
        );
        Self {
            win_score: weights.win_score,
            draw_score: weights.draw_score,
            ply_penalty: weights.ply_penalty,
            placement_step: weights.placement_step,
        }
    }

    /// The score of a melee that ended `ply` plies after the search root with
    /// `outcome`; a continuing outcome has no terminal score and panics.
    ///
    /// # Panics
    ///
    /// Panics on `MeleeOutcome::Continues`.
    #[must_use]
    pub fn score(&self, outcome: &MeleeOutcome, ply: u16) -> i32 {
        let ply = i32::from(ply.min(MAX_PLY));
        match *outcome {
            MeleeOutcome::WeAlone => self.win_score - self.ply_penalty * ply,
            MeleeOutcome::WeDown { rivals_left: 0 } => self.draw_score,
            MeleeOutcome::WeDown { rivals_left } => {
                let outlived = MAX_RIVALS - i32::from(rivals_left);
                -self.win_score + self.ply_penalty * ply + self.placement_step * outlived
            }
            MeleeOutcome::Continues(_) => panic!("a continuing melee has no terminal score"),
        }
    }

    /// A value strictly greater than every terminal score, for search windows.
    #[must_use]
    pub const fn sentinel(&self) -> i32 {
        self.win_score + 1
    }

    /// The smallest terminal magnitude: every non-terminal score must stay
    /// strictly inside `(-finite_limit, finite_limit)`.
    #[must_use]
    pub const fn finite_limit(&self) -> i32 {
        self.win_score - self.ply_penalty * MAX_PLY as i32 - self.placement_step * MAX_RIVALS
    }
}
