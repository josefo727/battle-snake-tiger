//! Every coefficient of the melee valuation and its terminal scores, by name.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MeleeWeights {
    /// Magnitude of being the last serpent alive (and, negated, of dying first).
    pub win_score: i32,
    /// Score of a wipe-out: every serpent dead in the same turn.
    pub draw_score: i32,
    /// Score lost per ply before a win, and gained per ply before a loss.
    pub ply_penalty: i32,
    /// Value of each opponent that is already gone when we die.
    pub placement_step: i32,
    /// Value of one cell of territory over the largest rival's.
    pub territory_cell: i32,
    /// Value of each segment over the longest rival.
    pub standing_segment: i32,
    /// Strength of the pull toward food as our health margin shrinks.
    pub hunger_urgency: i32,
    /// Value of each turn of nearness to the nearest pellet we reach first,
    /// whatever our health (growth iteration 2).
    pub appetite_step: i32,
    /// Value of each pellet we reach before every rival (growth iteration 6).
    pub larder_pellet: i32,
    /// Value of each next cell we would win or lose a head-to-head on.
    pub head_danger: i32,
    /// Value of each seat already eliminated.
    pub attrition_seat: i32,
}

/// The initial, reasoned (not yet fitted) profile; the placement benchmark
/// refines it.
pub const DEFAULT_MELEE_PROFILE: MeleeWeights = MeleeWeights {
    win_score: 1_000_000,
    draw_score: -10_000,
    ply_penalty: 100,
    placement_step: 20_000,
    territory_cell: 100,
    standing_segment: 1_000,
    hunger_urgency: 200,
    appetite_step: 40,
    larder_pellet: 400,
    head_danger: 300,
    attrition_seat: 5_000,
};
