//! Every coefficient of the valuation, by name, in one profile.
//!
//! Changing a weight never touches assessor code: an experiment builds a
//! `WeightSheet { field: value, ..DEFAULT_PROFILE }`. Values are integers so
//! scores are exact and reproducible.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WeightSheet {
    /// Magnitude of a forced win (and, negated, a forced loss).
    pub win_score: i32,
    /// Score of a mutual elimination.
    pub draw_score: i32,
    /// Score lost per ply before the end, so faster wins score higher.
    pub ply_penalty: i32,
    /// Value of one cell of territory reached first.
    pub territory_cell: i32,
    /// Strength of the pull toward food as the health margin shrinks.
    pub hunger_urgency: i32,
    /// Value of each segment of length advantage.
    pub length_advantage: i32,
    /// Value of head-to-head threat coverage while we are longer.
    pub head_pressure: i32,
    /// Value of each turn of estimated survival in a separated region.
    pub enclosure_turn: i32,
}

/// The initial, reasoned (not yet fitted) profile; sparring iterations refine it.
pub const DEFAULT_PROFILE: WeightSheet = WeightSheet {
    win_score: 1_000_000,
    draw_score: -1_000,
    ply_penalty: 100,
    territory_cell: 100,
    hunger_urgency: 200,
    length_advantage: 1_000,
    head_pressure: 150,
    enclosure_turn: 120,
};
