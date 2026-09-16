use proptest::prelude::*;
use tiger_engine::arena::melee::MeleeOutcome;
use tiger_engine::valuation::finish::MAX_PLY;
use tiger_engine::valuation::melee::finish::MeleeFinish;
use tiger_engine::valuation::melee::weights::{DEFAULT_MELEE_PROFILE, MeleeWeights};

fn finish() -> MeleeFinish {
    MeleeFinish::new(&DEFAULT_MELEE_PROFILE)
}

fn down(rivals_left: u8) -> MeleeOutcome {
    MeleeOutcome::WeDown { rivals_left }
}

#[test]
fn being_last_alive_scores_the_win_less_the_plies_it_took() {
    let f = finish();

    assert_eq!(f.score(&MeleeOutcome::WeAlone, 1), 1_000_000 - 100);
    assert_eq!(f.score(&MeleeOutcome::WeAlone, 7), 1_000_000 - 700);
    assert!(f.score(&MeleeOutcome::WeAlone, 3) > f.score(&MeleeOutcome::WeAlone, 4));
}

#[test]
fn dying_scores_worse_the_earlier_and_the_more_rivals_outlive_us() {
    let f = finish();

    // Dying first among four: the full loss, plus one point of ply survived.
    assert_eq!(f.score(&down(3), 1), -1_000_000 + 100);
    // Dying second: one placement step better; third: two steps.
    assert_eq!(f.score(&down(2), 1), -1_000_000 + 100 + 20_000);
    assert_eq!(f.score(&down(1), 1), -1_000_000 + 100 + 40_000);
    assert!(f.score(&down(3), 5) > f.score(&down(3), 4));
    assert!(f.score(&down(2), 1) > f.score(&down(3), 200));
}

#[test]
fn a_wipe_out_is_the_draw_score_whatever_the_ply() {
    let f = finish();

    assert_eq!(f.score(&down(0), 1), -10_000);
    assert_eq!(f.score(&down(0), 40), -10_000);
}

#[test]
fn plies_beyond_the_horizon_score_as_the_horizon() {
    let f = finish();

    assert_eq!(
        f.score(&MeleeOutcome::WeAlone, MAX_PLY + 50),
        f.score(&MeleeOutcome::WeAlone, MAX_PLY)
    );
    assert_eq!(f.score(&down(3), MAX_PLY + 50), f.score(&down(3), MAX_PLY));
}

#[test]
#[should_panic(expected = "a forced win at MAX_PLY must still outscore")]
fn weights_that_let_a_win_cross_zero_are_refused() {
    let _ = MeleeFinish::new(&MeleeWeights {
        win_score: 50_000,
        ..DEFAULT_MELEE_PROFILE
    });
}

proptest! {
    #[test]
    fn terminal_scores_stay_between_the_finite_limit_and_the_sentinel(
        ply in 0u16..600, rivals in 1u8..=3,
    ) {
        let f = finish();
        let win = f.score(&MeleeOutcome::WeAlone, ply);
        let loss = f.score(&down(rivals), ply);
        prop_assert!(win >= f.finite_limit() && win < f.sentinel());
        prop_assert!(loss <= -f.finite_limit() && loss > -f.sentinel());
        prop_assert!(f.score(&down(0), ply).abs() < f.finite_limit());
        prop_assert!(f.finite_limit() > 0);
    }
}
