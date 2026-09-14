use proptest::prelude::*;
use tiger_sparring::statistics::{
    StatisticsError, Summary, Tally, summarize, wilson_interval, win_rate,
};

const TOLERANCE: f64 = 1e-9;

fn close(actual: f64, expected: f64) -> bool {
    (actual - expected).abs() <= TOLERANCE
}

fn tally(wins: u32, losses: u32, draws: u32) -> Tally {
    Tally {
        wins,
        losses,
        draws,
    }
}

// Reference values computed independently with 50-digit decimal arithmetic from
// the closed form (z = 1.959963984540054).
const REFERENCE: [(u32, u32, f64, f64); 8] = [
    (0, 30, 0.0, 0.113_513_393_173_969),
    (15, 30, 0.331_541_256_405_338, 0.668_458_743_594_662),
    (30, 30, 0.886_486_606_826_031, 1.0),
    (20, 30, 0.487_800_516_445_438, 0.807_695_019_163_239),
    (1, 1, 0.206_549_314_377_237, 1.0),
    (0, 1, 0.0, 0.793_450_685_622_763),
    (7, 10, 0.396_778_147_461_145, 0.892_208_732_593_699),
    (29, 30, 0.833_296_090_085_908, 0.994_091_409_618_388),
];

#[test]
fn the_wilson_interval_matches_the_reference_values() {
    for (wins, games, low, high) in REFERENCE {
        let (lower, upper) = wilson_interval(wins, games).expect("a non-empty sample");

        assert!(close(lower, low), "{wins}/{games} lower: {lower} vs {low}");
        assert!(
            close(upper, high),
            "{wins}/{games} upper: {upper} vs {high}"
        );
    }
}

#[test]
fn the_win_rate_counts_only_wins_over_every_game() {
    assert!(close(win_rate(&tally(15, 15, 0)).unwrap(), 0.5));
    assert!(
        close(win_rate(&tally(10, 10, 10)).unwrap(), 1.0 / 3.0),
        "draws stay in the denominator"
    );
    assert!(
        close(win_rate(&tally(0, 0, 5)).unwrap(), 0.0),
        "a draw is not a win"
    );
    assert!(close(win_rate(&tally(30, 0, 0)).unwrap(), 1.0));
}

#[test]
fn draws_are_reported_separately_from_wins_and_losses() {
    let summary: Summary = summarize(&tally(12, 15, 3)).unwrap();

    assert_eq!(
        (
            summary.tally.wins,
            summary.tally.losses,
            summary.tally.draws
        ),
        (12, 15, 3)
    );
    assert!(close(summary.win_rate, 0.4));
    let (lower, upper) = wilson_interval(12, 30).unwrap();
    assert!(close(summary.wilson95.0, lower) && close(summary.wilson95.1, upper));
}

#[test]
fn an_empty_sample_is_rejected_with_a_typed_error() {
    assert_eq!(win_rate(&tally(0, 0, 0)), Err(StatisticsError::EmptySample));
    assert_eq!(wilson_interval(0, 0), Err(StatisticsError::EmptySample));
    assert_eq!(
        summarize(&tally(0, 0, 0)),
        Err(StatisticsError::EmptySample)
    );
}

#[test]
fn more_wins_than_games_is_rejected() {
    assert_eq!(
        wilson_interval(31, 30),
        Err(StatisticsError::WinsExceedGames)
    );
}

#[test]
fn a_tally_totals_wins_losses_and_draws() {
    assert_eq!(tally(3, 4, 5).total(), 12);
}

proptest! {
    #[test]
    fn the_interval_is_ordered_inside_the_unit_range_and_contains_the_rate(games in 1u32..500, share in 0.0f64..=1.0) {
        let wins = (f64::from(games) * share).round() as u32;

        let (lower, upper) = wilson_interval(wins, games).unwrap();

        let rate = f64::from(wins) / f64::from(games);
        prop_assert!((0.0..=1.0).contains(&lower) && (0.0..=1.0).contains(&upper));
        prop_assert!(lower <= rate + 1e-12 && rate <= upper + 1e-12);
        prop_assert!(lower <= upper);
    }

    #[test]
    fn the_interval_is_symmetric_between_wins_and_losses(games in 1u32..500, wins in 0u32..500) {
        let wins = wins.min(games);

        let (lower, upper) = wilson_interval(wins, games).unwrap();
        let (mirror_lower, mirror_upper) = wilson_interval(games - wins, games).unwrap();

        prop_assert!(close(lower, 1.0 - mirror_upper) && close(upper, 1.0 - mirror_lower));
    }

    #[test]
    fn more_games_at_the_same_rate_narrow_the_interval(games in 2u32..200, share in 0.05f64..0.95) {
        let wins = (f64::from(games) * share).round() as u32;
        let (lower, upper) = wilson_interval(wins, games).unwrap();
        let (lower4, upper4) = wilson_interval(wins * 4, games * 4).unwrap();

        prop_assert!(upper4 - lower4 < upper - lower);
    }
}
