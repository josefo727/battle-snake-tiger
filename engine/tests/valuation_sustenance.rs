mod support;

use proptest::prelude::*;
use tiger_engine::arena::cellset::CellSet;
use tiger_engine::arena::duel::{DuelBoard, Side};
use tiger_engine::arena::ingest::ingest;
use tiger_engine::valuation::dominion::Dominion;
use tiger_engine::valuation::sustenance::{
    COMFORT_MARGIN, MAX_PRESSURE, NO_FOOD_DISTANCE, Sustenance,
};
use tiger_engine::valuation::{Assessor, ValuationPipeline};

use support::{realize, state_spec, turn_state_from_bodies};

const US: &[(i32, i32)] = &[(5, 5), (5, 4), (5, 3)];
const FAR_CORNER: &[(i32, i32)] = &[(0, 0), (1, 0), (2, 0)];
const NEAR_THEM: &[(i32, i32)] = &[(5, 10), (4, 10), (3, 10)];

fn board(
    us: &[(i32, i32)],
    them: &[(i32, i32)],
    vigor: [i32; 2],
    food: &[(i32, i32)],
) -> DuelBoard {
    ingest(&turn_state_from_bodies(&[us, them], &vigor, 0, food)).expect("a duel converts")
}

fn swapped(board: &DuelBoard) -> DuelBoard {
    DuelBoard::try_new(
        *board.serpent(Side::Them),
        *board.serpent(Side::Us),
        board.pellets(),
    )
    .expect("swapping two valid serpents stays valid")
}

#[test]
fn the_constants_have_the_documented_scale() {
    assert_eq!(COMFORT_MARGIN, 25);
    assert_eq!(NO_FOOD_DISTANCE, 40);
}

#[test]
fn a_hungry_serpent_with_food_only_it_can_reach_is_under_pressure() {
    // Our head (5, 5) reaches the pellet at (5, 10) in 5 turns; theirs is far
    // away. Our margin is 10 - 5 = 5, so our pressure is 25 - 5 = 20; they hold
    // 90 health and no owned pellet (margin 90 - 40 = 50), so none.
    let b = board(US, FAR_CORNER, [10, 90], &[(5, 10)]);

    assert_eq!(Sustenance.assess(&b), -20);
}

#[test]
fn comfortable_health_produces_no_pressure_for_either_serpent() {
    let b = board(US, FAR_CORNER, [100, 100], &[(5, 10)]);

    assert_eq!(Sustenance.assess(&b), 0);
}

#[test]
fn pressure_is_capped_so_starvation_cannot_swamp_the_rest_of_the_score() {
    // Health 1 with no pellet of our own: margin 1 - 40 = -39, an uncapped
    // pressure of 64; the cap holds it at MAX_PRESSURE.
    let starving = board(US, FAR_CORNER, [1, 100], &[]);
    let both_starving = board(US, FAR_CORNER, [1, 1], &[]);

    assert_eq!(MAX_PRESSURE, 60);
    assert_eq!(Sustenance.assess(&starving), -MAX_PRESSURE);
    assert_eq!(Sustenance.assess(&both_starving), 0);
}

#[test]
fn the_other_serpents_hunger_is_our_advantage() {
    let b = board(US, FAR_CORNER, [100, 30], &[(2, 6)]);

    assert!(Sustenance.assess(&b) > 0, "they are hungrier than we are");
}

#[test]
fn a_pellet_the_other_serpent_reaches_first_gives_us_no_relief() {
    // (5, 9) is one step from their head but four from ours, so it is theirs.
    let only_theirs = board(US, NEAR_THEM, [10, 90], &[(5, 9)]);
    // (3, 4) is three steps from our head and eight from theirs: ours, and
    // nearer to us than their pellet, which must not be counted for us.
    let with_our_own = board(US, NEAR_THEM, [10, 90], &[(5, 9), (3, 4)]);

    assert_eq!(Dominion.survey(&only_theirs).our_food, None);
    assert_eq!(Dominion.survey(&with_our_own).our_food, Some(3));
    assert!(Sustenance.assess(&only_theirs) < Sustenance.assess(&with_our_own));
    assert_eq!(Sustenance.assess(&only_theirs), -55);
}

#[test]
fn pressure_never_drops_as_our_health_falls() {
    let mut previous: Option<i32> = None;
    for vigor in (1..=100).rev() {
        let score = Sustenance.assess(&board(US, FAR_CORNER, [vigor, 100], &[(5, 10)]));
        if let Some(before) = previous {
            assert!(
                score <= before,
                "vigor {vigor}: {score} must not exceed {before}"
            );
        }
        previous = Some(score);
    }

    let low = Sustenance.assess(&board(US, FAR_CORNER, [12, 100], &[(5, 10)]));
    let lower = Sustenance.assess(&board(US, FAR_CORNER, [8, 100], &[(5, 10)]));
    assert!(
        lower < low,
        "less health means strictly more pressure inside the comfort band"
    );
}

#[test]
fn the_survey_reports_the_nearest_owned_pellet_and_agrees_with_the_partition() {
    let b = board(US, FAR_CORNER, [90, 90], &[(5, 10), (10, 10)]);

    let survey = Dominion.survey(&b);

    assert_eq!(survey.our_food, Some(5));
    assert_eq!(survey.their_food, None);
    assert_eq!(survey.partition, Dominion.partition(&b));
}

#[test]
fn a_pellet_reached_on_the_same_turn_belongs_to_the_longer_serpent_only() {
    let equal_us: &[(i32, i32)] = &[(0, 5), (0, 6), (0, 7)];
    let equal_them: &[(i32, i32)] = &[(10, 5), (10, 6), (10, 7)];
    let equal = board(equal_us, equal_them, [90, 90], &[(5, 5)]);
    let longer = board(
        &[(0, 5), (0, 6), (0, 7), (0, 8)],
        equal_them,
        [90, 90],
        &[(5, 5)],
    );

    assert_eq!(Dominion.survey(&equal).our_food, None);
    assert_eq!(Dominion.survey(&equal).their_food, None);
    assert_eq!(Dominion.survey(&longer).our_food, Some(5));
    assert_eq!(Dominion.survey(&longer).their_food, None);
}

#[test]
fn sustenance_weights_through_the_pipeline() {
    let b = board(US, FAR_CORNER, [10, 90], &[(5, 10)]);

    assert_eq!(
        ValuationPipeline::empty().with(Sustenance, 200).score(&b),
        -4_000
    );
}

proptest! {
    #[test]
    fn swapping_the_serpents_negates_the_contribution(spec in state_spec()) {
        let b = ingest(&realize(&spec)).expect("a duel converts");

        prop_assert_eq!(Sustenance.assess(&swapped(&b)), -Sustenance.assess(&b));
    }

    #[test]
    fn the_survey_only_reports_food_inside_the_owned_territory(spec in state_spec()) {
        let b = ingest(&realize(&spec)).expect("a duel converts");
        let survey = Dominion.survey(&b);
        let owned_pellets = |set: CellSet| set.intersection(b.pellets());

        prop_assert_eq!(survey.our_food.is_some(), owned_pellets(survey.partition.ours) != CellSet::EMPTY);
        prop_assert_eq!(survey.their_food.is_some(), owned_pellets(survey.partition.theirs) != CellSet::EMPTY);
    }
}
