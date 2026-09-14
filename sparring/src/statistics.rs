//! Win rates and their 95% Wilson score intervals.

/// Games won, lost and drawn by the challenger against one opponent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tally {
    pub wins: u32,
    pub losses: u32,
    pub draws: u32,
}

impl Tally {
    #[must_use]
    pub const fn total(&self) -> u32 {
        self.wins + self.losses + self.draws
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StatisticsError {
    /// No games were played, so there is no rate to report.
    EmptySample,
    /// More wins than games: the counts cannot describe one benchmark.
    WinsExceedGames,
}

/// The 97.5th percentile of the standard normal distribution, for two-sided 95%.
const Z_95: f64 = 1.959_963_984_540_054;

/// A tally with its rate and interval.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Summary {
    pub tally: Tally,
    pub win_rate: f64,
    pub wilson95: (f64, f64),
}

/// Wins over all games played; a draw is not a win and stays in the denominator.
///
/// # Errors
///
/// Fails when no games were played.
pub fn win_rate(tally: &Tally) -> Result<f64, StatisticsError> {
    match tally.total() {
        0 => Err(StatisticsError::EmptySample),
        games => Ok(f64::from(tally.wins) / f64::from(games)),
    }
}

/// The 95% Wilson score interval for `wins` successes in `games` trials.
///
/// # Errors
///
/// Fails when `games` is zero or `wins` exceeds it.
pub fn wilson_interval(wins: u32, games: u32) -> Result<(f64, f64), StatisticsError> {
    if games == 0 {
        return Err(StatisticsError::EmptySample);
    }
    if wins > games {
        return Err(StatisticsError::WinsExceedGames);
    }
    let n = f64::from(games);
    let rate = f64::from(wins) / n;
    let z_squared = Z_95 * Z_95;
    let shrink = 1.0 + z_squared / n;
    let centre = (rate + z_squared / (2.0 * n)) / shrink;
    let half_width = Z_95 * (rate * (1.0 - rate) / n + z_squared / (4.0 * n * n)).sqrt() / shrink;
    // Rounding can leave the ends a hair outside [0, 1] when the rate is 0 or 1.
    Ok((
        (centre - half_width).max(0.0),
        (centre + half_width).min(1.0),
    ))
}

/// The rate and interval for a tally.
///
/// # Errors
///
/// Fails when no games were played.
pub fn summarize(tally: &Tally) -> Result<Summary, StatisticsError> {
    Ok(Summary {
        tally: *tally,
        win_rate: win_rate(tally)?,
        wilson95: wilson_interval(tally.wins, tally.total())?,
    })
}
