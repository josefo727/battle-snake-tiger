//! The order in which a layer tries its four headings.
//!
//! Alpha-beta prunes most when the best heading comes first, so the search asks a
//! [`HeadingOrder`] for the order at every layer and tells it about cutoffs. The
//! order only changes how much work is done, never the value found.

use std::cmp::Reverse;

use crate::arena::heading::Heading;
use crate::arena::melee::{MAX_SEATS, Seat};

/// Seats are the key of every table: a duel `Side` converts to one (`Us` is
/// seat 0, `Them` seat 1), so the duel search calls these with sides.
pub trait HeadingOrder {
    /// The four headings for `seat` at search ply `ply`, best guess first.
    fn arrange(&self, seat: impl Into<Seat>, ply: u16) -> [Heading; 4];

    /// `heading` produced a cutoff for `seat` at `ply` with `depth` plies left.
    fn note_cutoff(&mut self, seat: impl Into<Seat>, ply: u16, heading: Heading, depth: u16);

    /// A completed search chose `heading` at the root.
    fn note_root_best(&mut self, heading: Heading);
}

/// Always the fixed heading order; the baseline the learned order is measured
/// against.
#[derive(Clone, Copy, Debug, Default)]
pub struct NaturalOrder;

impl HeadingOrder for NaturalOrder {
    fn arrange(&self, _seat: impl Into<Seat>, _ply: u16) -> [Heading; 4] {
        Heading::ALL
    }

    fn note_cutoff(&mut self, _seat: impl Into<Seat>, _ply: u16, _heading: Heading, _depth: u16) {}

    fn note_root_best(&mut self, _heading: Heading) {}
}

/// Plies tracked for killers; deeper plies share the last slot.
const TRACKED_PLIES: usize = 64;
const KILLERS_PER_SLOT: usize = 2;

/// Previous best first, then killers for the ply, then history-ranked headings.
///
/// - The previous best is the root heading of the last completed search; it leads
///   only our root layer, where deepening makes it the strongest guess.
/// - A killer is a heading that recently produced a cutoff for the same seat at
///   the same ply; the two most recent are kept.
/// - History adds `depth * depth` per cutoff, by seat and heading, so cutoffs with
///   more plies left weigh more; ties fall back to the fixed heading order.
#[derive(Clone, Debug)]
pub struct LearnedOrder {
    previous_best: Option<Heading>,
    killers: [[[Option<Heading>; KILLERS_PER_SLOT]; MAX_SEATS]; TRACKED_PLIES],
    history: [[u32; 4]; MAX_SEATS],
}

impl LearnedOrder {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            previous_best: None,
            killers: [[[None; KILLERS_PER_SLOT]; MAX_SEATS]; TRACKED_PLIES],
            history: [[0; 4]; MAX_SEATS],
        }
    }

    fn slot(ply: u16) -> usize {
        usize::from(ply).min(TRACKED_PLIES - 1)
    }
}

impl Default for LearnedOrder {
    fn default() -> Self {
        Self::new()
    }
}

impl HeadingOrder for LearnedOrder {
    fn arrange(&self, seat: impl Into<Seat>, ply: u16) -> [Heading; 4] {
        let seat = seat.into();
        let mut lineup = Lineup::default();
        if seat == Seat::US && ply == 0 {
            lineup.push_all(self.previous_best);
        }
        lineup.push_all(
            self.killers[Self::slot(ply)][seat.index()]
                .into_iter()
                .flatten(),
        );
        let history = &self.history[seat.index()];
        let mut by_history = Heading::ALL;
        by_history.sort_unstable_by_key(|h| (Reverse(history[h.index()]), h.index()));
        lineup.push_all(by_history);
        lineup.headings
    }

    fn note_cutoff(&mut self, seat: impl Into<Seat>, ply: u16, heading: Heading, depth: u16) {
        let seat = seat.into();
        let killers = &mut self.killers[Self::slot(ply)][seat.index()];
        if killers[0] != Some(heading) {
            killers[1] = killers[0];
            killers[0] = Some(heading);
        }
        let weight = u32::from(depth) * u32::from(depth);
        let score = &mut self.history[seat.index()][heading.index()];
        *score = score.saturating_add(weight);
    }

    fn note_root_best(&mut self, heading: Heading) {
        self.previous_best = Some(heading);
    }
}

/// An arrangement under construction: each heading is admitted once, in the
/// order offered, and the rest of the four stay in fixed order until placed.
struct Lineup {
    headings: [Heading; 4],
    len: usize,
}

impl Default for Lineup {
    fn default() -> Self {
        Self {
            headings: Heading::ALL,
            len: 0,
        }
    }
}

impl Lineup {
    fn push_all(&mut self, offered: impl IntoIterator<Item = Heading>) {
        for heading in offered {
            if !self.headings[..self.len].contains(&heading) {
                self.headings[self.len] = heading;
                self.len += 1;
            }
        }
    }
}
