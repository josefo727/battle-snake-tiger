//! A last word for the positions the paranoid search has given up on.
//!
//! The search assumes every rival plays the worst line for us on every turn.
//! On a crowded board that model finds a forced loss almost everywhere, and
//! then every heading scores the same kind of nothing: on the ladder on
//! 2026-09-25 it called a melee lost from turn 105, and the move for the four
//! turns that followed was settled by the fixed heading order among headings it
//! had declared equally lost.
//!
//! Rivals do not in fact play that line, so when the verdict is a loss this
//! asks a different question, the one a rollout can answer and a search cannot:
//! from each heading, play the position out many times with every serpent --
//! ourselves included -- stepping evenly among the steps that do not kill it on
//! the spot, and count how long we last. A random walker dies in a corridor a
//! careful serpent would survive, so this reads open ground as worth more than
//! it is; the bias falls the same way on every heading of a position, and it is
//! the only reading available that sees as far as the mistake.
//!
//! Everything here is integer and seeded, so the same position gives the same
//! answer every time: a decision the engine cannot reproduce is one nobody can
//! argue with afterwards.

use super::allowance::StopSignal;
use crate::arena::cellset::CellSet;
use crate::arena::heading::Heading;
use crate::arena::melee::{MAX_SEATS, MeleeBoard, MeleeOutcome, Seat};

/// Turns a rollout plays before it gives up and calls us a survivor.
pub const HORIZON: u16 = 30;
/// Rollouts behind each heading.
pub const ROLLOUTS: u32 = 200;
/// The fixed seed. Two engines can only be compared on this number, and one
/// decision can only be explained afterwards, if it is the same number every
/// time it is asked.
pub const SEED: u64 = 0x5f2d_1c9b_a733_4e11;

/// The heading that lasts longest over `ROLLOUTS` plays of `HORIZON` turns, or
/// `None` when `stop` says to give up before every heading has been weighed.
///
/// Headings are weighed in the fixed order, so a tie keeps the order the rest
/// of the engine uses.
pub fn best_heading(board: &MeleeBoard, stop: &mut impl StopSignal) -> Option<Heading> {
    let mut best: Option<(Heading, u32)> = None;
    for heading in Heading::ALL {
        let survived = survival(board, heading, stop)?;
        if best.is_none_or(|(_, most)| survived > most) {
            best = Some((heading, survived));
        }
    }
    best.map(|(heading, _)| heading)
}

/// Turns we last, summed over every rollout, from `board` after playing
/// `heading`; `None` at the stop signal.
pub fn survival(board: &MeleeBoard, heading: Heading, stop: &mut impl StopSignal) -> Option<u32> {
    let mut rng = Xorshift::new(SEED);
    let mut total = 0;
    for _ in 0..ROLLOUTS {
        if stop.should_stop() {
            return None;
        }
        total += u32::from(one_rollout(board, heading, &mut rng));
    }
    Some(total)
}

/// Turns we last from `board` after playing `heading`, capped at [`HORIZON`].
fn one_rollout(board: &MeleeBoard, heading: Heading, rng: &mut Xorshift) -> u16 {
    let mut current = *board;
    let mut first = Some(heading);
    for turn in 0..HORIZON {
        let mut moves = [Heading::ALL[0]; MAX_SEATS];
        let enterable = enterable_cells(&current);
        for seat in current.seats() {
            moves[seat.index()] = if seat == Seat::US && first.is_some() {
                first.take().unwrap_or(heading)
            } else {
                safe_random_step(&current, seat, enterable, rng)
            };
        }
        match current.advance(&moves) {
            MeleeOutcome::Continues(next) => current = next,
            MeleeOutcome::WeAlone => return HORIZON,
            MeleeOutcome::WeDown { .. } => return turn,
        }
    }
    HORIZON
}

/// A step for `seat` drawn evenly from those that do not kill it on the spot;
/// when every step does, the first of them, because it makes no difference.
fn safe_random_step(
    board: &MeleeBoard,
    seat: Seat,
    enterable: CellSet,
    rng: &mut Xorshift,
) -> Heading {
    let head = board.serpent(seat).head();
    let mut safe = [Heading::ALL[0]; 4];
    let mut count = 0;
    for heading in Heading::ALL {
        if heading
            .step(head)
            .is_some_and(|cell| enterable.contains(cell))
        {
            safe[count] = heading;
            count += 1;
        }
    }
    if count == 0 {
        return Heading::ALL[0];
    }
    safe[(rng.next() % count as u64) as usize]
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

/// The smallest reproducible source of randomness that will do the job.
struct Xorshift(u64);

impl Xorshift {
    const fn new(seed: u64) -> Self {
        Self(seed | 1)
    }

    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}
