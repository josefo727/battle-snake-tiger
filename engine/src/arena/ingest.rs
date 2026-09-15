//! The one bridge from the reused rules core's `TurnState` into the kernel.

use super::cellset::{Cell, CellSet};
use super::duel::{BoardError, DuelBoard};
use super::heading::Heading;
use super::melee::MeleeBoard;
use super::serpent::{Serpent, SerpentError};
use crate::rules_core::{Direction, SnakeState, TurnState};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IngestError {
    NotADuel,
    /// A melee has three or four snakes.
    NotAMelee,
    CellOutOfRange,
    Serpent(SerpentError),
    Board(BoardError),
}

/// Converts a reused `TurnState` into a `DuelBoard` with our snake first.
///
/// # Errors
///
/// Fails unless the state has exactly two snakes with valid bodies.
pub fn ingest(state: &TurnState) -> Result<DuelBoard, IngestError> {
    let snakes = state.snakes();
    if snakes.len() != 2 {
        return Err(IngestError::NotADuel);
    }

    let us = to_serpent(state.you())?;
    let them = to_serpent(&snakes[1 - state.you_index()])?;
    // The reused board mask and this kernel number cells identically (y * 11 + x).
    let pellets = CellSet::from_bits(state.food().bits());

    DuelBoard::try_new(us, them, pellets).map_err(IngestError::Board)
}

/// Converts a reused `TurnState` with three or four snakes into a `MeleeBoard`
/// with our snake in seat 0 and the others in their original order.
///
/// # Errors
///
/// Fails unless the state has three or four snakes with valid bodies.
pub fn ingest_melee(state: &TurnState) -> Result<MeleeBoard, IngestError> {
    let snakes = state.snakes();
    if !(3..=4).contains(&snakes.len()) {
        return Err(IngestError::NotAMelee);
    }
    let mut serpents = vec![to_serpent(state.you())?];
    for (index, snake) in snakes.iter().enumerate() {
        if index != state.you_index() {
            serpents.push(to_serpent(snake)?);
        }
    }
    let pellets = CellSet::from_bits(state.food().bits());
    MeleeBoard::try_new(&serpents, pellets).map_err(IngestError::Board)
}

/// The wire direction for a kernel heading: the one bridge out of the kernel,
/// mirroring [`ingest`] on the way in.
#[must_use]
pub const fn direction_of(heading: Heading) -> Direction {
    match heading {
        Heading::North => Direction::Up,
        Heading::East => Direction::Right,
        Heading::South => Direction::Down,
        Heading::West => Direction::Left,
    }
}

fn to_serpent(snake: &SnakeState) -> Result<Serpent, IngestError> {
    let body = snake
        .body()
        .iter()
        .map(|cell| Cell::from_index(cell.value()).ok_or(IngestError::CellOutOfRange))
        .collect::<Result<Vec<_>, _>>()?;

    Serpent::new(&body, snake.health()).map_err(IngestError::Serpent)
}
