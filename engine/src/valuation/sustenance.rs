//! Hunger: how much closer to starving each serpent is than its nearest food.

use super::Assessor;
use super::dominion::Dominion;
use crate::arena::duel::{DuelBoard, Side};

/// A serpent whose health exceeds its distance to food by more than this many
/// turns feels no pressure.
pub const COMFORT_MARGIN: i32 = 25;

/// The distance assumed when a serpent owns no pellet at all.
pub const NO_FOOD_DISTANCE: i32 = 40;

/// Pressure stops growing here: past this much hunger a serpent is already in
/// trouble and a larger number would only crowd out the rest of the score.
pub const MAX_PRESSURE: i32 = 60;

#[derive(Clone, Copy, Debug, Default)]
pub struct Sustenance;

/// `max(0, COMFORT_MARGIN - (health - distance))`: it grows as the margin
/// between a serpent's health and the turns to its nearest owned pellet shrinks
/// (and is largest once it cannot reach food before starving).
fn pressure(vigor: u8, distance_to_food: Option<u16>) -> i32 {
    let distance = distance_to_food.map_or(NO_FOOD_DISTANCE, i32::from);
    let margin = i32::from(vigor) - distance;
    (COMFORT_MARGIN - margin).clamp(0, MAX_PRESSURE)
}

impl Assessor for Sustenance {
    type Board = DuelBoard;

    const NAME: &'static str = "sustenance";
    const MAX_RAW: i32 = MAX_PRESSURE;

    /// Their pressure minus ours: positive when they are the hungrier serpent.
    fn assess(&self, board: &DuelBoard) -> i32 {
        let survey = Dominion.survey(board);
        let ours = pressure(board.serpent(Side::Us).vigor(), survey.our_food);
        let theirs = pressure(board.serpent(Side::Them).vigor(), survey.their_food);
        theirs - ours
    }
}
