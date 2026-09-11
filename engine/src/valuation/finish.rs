//! Terminal scoring: what a finished duel is worth, with a ply-distance term so
//! faster wins beat slower wins and slower losses beat faster ones.

use super::weights::WeightSheet;
use crate::arena::duel::Verdict;

/// The deepest ply the distance term distinguishes; later plies score as this
/// one, so a terminal score can never change sign or drop below the limit.
pub const MAX_PLY: u16 = 250;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Finish {
    win_score: i32,
    draw_score: i32,
    ply_penalty: i32,
}

impl Finish {
    /// # Panics
    ///
    /// Panics when the weights would let a forced win at `MAX_PLY` score at or
    /// below zero, which would invert the ordering search relies on.
    #[must_use]
    pub const fn new(weights: &WeightSheet) -> Self {
        assert!(
            weights.win_score > weights.ply_penalty * MAX_PLY as i32,
            "a forced win at MAX_PLY must still outscore any draw or position"
        );
        Self {
            win_score: weights.win_score,
            draw_score: weights.draw_score,
            ply_penalty: weights.ply_penalty,
        }
    }

    /// The score of a finished duel that ended `ply` plies after the search root.
    #[must_use]
    pub const fn score(&self, verdict: Verdict, ply: u16) -> i32 {
        let ply = if ply > MAX_PLY { MAX_PLY } else { ply };
        let win = self.win_score - self.ply_penalty * ply as i32;
        match verdict {
            Verdict::WeOnly => win,
            Verdict::TheyOnly => -win,
            Verdict::BothDown => self.draw_score,
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
        self.win_score - self.ply_penalty * MAX_PLY as i32
    }
}
