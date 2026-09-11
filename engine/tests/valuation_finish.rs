use proptest::prelude::*;
use tiger_engine::arena::duel::Verdict;
use tiger_engine::valuation::finish::{Finish, MAX_PLY};
use tiger_engine::valuation::weights::{DEFAULT_PROFILE, WeightSheet};

fn finish() -> Finish {
    Finish::new(&DEFAULT_PROFILE)
}

#[test]
fn a_win_is_positive_a_loss_is_its_negation_and_a_double_loss_is_the_draw_score() {
    let f = finish();

    assert_eq!(
        f.score(Verdict::WeOnly, 3),
        DEFAULT_PROFILE.win_score - 3 * DEFAULT_PROFILE.ply_penalty
    );
    assert_eq!(f.score(Verdict::TheyOnly, 3), -f.score(Verdict::WeOnly, 3));
    assert_eq!(f.score(Verdict::BothDown, 3), DEFAULT_PROFILE.draw_score);
    assert_eq!(f.score(Verdict::BothDown, 90), DEFAULT_PROFILE.draw_score);
}

#[test]
fn a_win_at_the_root_is_worth_the_full_win_score() {
    assert_eq!(
        finish().score(Verdict::WeOnly, 0),
        DEFAULT_PROFILE.win_score
    );
}

#[test]
fn ply_distances_beyond_the_maximum_score_like_the_maximum() {
    let f = finish();

    assert_eq!(
        f.score(Verdict::WeOnly, u16::MAX),
        f.score(Verdict::WeOnly, MAX_PLY)
    );
    assert!(f.score(Verdict::WeOnly, u16::MAX) > 0);
    assert_eq!(
        f.score(Verdict::TheyOnly, u16::MAX),
        f.score(Verdict::TheyOnly, MAX_PLY)
    );
}

#[test]
fn custom_weights_change_the_scale() {
    let sheet = WeightSheet {
        win_score: 5_000,
        ply_penalty: 10,
        draw_score: -7,
        ..DEFAULT_PROFILE
    };
    let f = Finish::new(&sheet);

    assert_eq!(f.score(Verdict::WeOnly, 4), 4_960);
    assert_eq!(f.score(Verdict::BothDown, 4), -7);
    assert_eq!(f.sentinel(), 5_001);
}

#[test]
#[should_panic(expected = "a forced win at MAX_PLY must still outscore")]
fn weights_that_let_a_late_win_reach_zero_are_rejected() {
    let sheet = WeightSheet {
        win_score: 100,
        ply_penalty: 10,
        ..DEFAULT_PROFILE
    };

    let _ = Finish::new(&sheet);
}

#[test]
fn the_sentinel_and_the_finite_limit_bracket_every_terminal_score() {
    let f = finish();

    assert_eq!(f.sentinel(), DEFAULT_PROFILE.win_score + 1);
    assert_eq!(
        f.finite_limit(),
        DEFAULT_PROFILE.win_score - DEFAULT_PROFILE.ply_penalty * i32::from(MAX_PLY)
    );
    assert!(f.finite_limit() > 0);
}

fn any_ply() -> impl Strategy<Value = u16> {
    0u16..=u16::MAX
}

proptest! {
    #[test]
    fn faster_wins_score_higher_and_slower_losses_score_higher(
        a in 0u16..=MAX_PLY + 20,
        b in 0u16..=MAX_PLY + 20,
    ) {
        let f = finish();
        let (fast, slow) = if a < b { (a, b) } else { (b, a) };
        // Equal plies tie, and plies at or beyond MAX_PLY clamp to one score by design.
        prop_assume!(fast < slow && fast < MAX_PLY);

        prop_assert!(f.score(Verdict::WeOnly, fast) > f.score(Verdict::WeOnly, slow));
        prop_assert!(f.score(Verdict::TheyOnly, slow) > f.score(Verdict::TheyOnly, fast));
    }

    #[test]
    fn every_win_beats_the_draw_and_every_draw_beats_every_loss(ply in any_ply(), other in any_ply()) {
        let f = finish();

        prop_assert!(f.score(Verdict::WeOnly, ply) > f.score(Verdict::BothDown, other));
        prop_assert!(f.score(Verdict::BothDown, ply) > f.score(Verdict::TheyOnly, other));
    }

    #[test]
    fn terminal_scores_stay_between_the_finite_limit_and_the_sentinel(ply in any_ply()) {
        let f = finish();
        let win = f.score(Verdict::WeOnly, ply);
        let loss = f.score(Verdict::TheyOnly, ply);

        prop_assert!(win >= f.finite_limit() && win < f.sentinel());
        prop_assert!(loss <= -f.finite_limit() && loss > -f.sentinel());
        prop_assert!(f.score(Verdict::BothDown, ply).abs() < f.finite_limit());
    }
}
