//! The one bridge from the reused rules core's `TurnState` into the kernel.

use super::cellset::{Cell, CellSet};
use super::duel::{BoardError, DuelBoard};
use super::serpent::{Serpent, SerpentError};
use crate::rules_core::{SnakeState, TurnState};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IngestError {
    NotADuel,
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

fn to_serpent(snake: &SnakeState) -> Result<Serpent, IngestError> {
    let body = snake
        .body()
        .iter()
        .map(|cell| Cell::from_index(cell.value()).ok_or(IngestError::CellOutOfRange))
        .collect::<Result<Vec<_>, _>>()?;

    Serpent::new(&body, snake.health()).map_err(IngestError::Serpent)
}
