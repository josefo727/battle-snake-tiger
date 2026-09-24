mod support;

use proptest::prelude::*;
use tiger_engine::arena::heading::Heading;
use tiger_engine::arena::ingest::ingest_melee;
use tiger_engine::arena::melee::{MAX_SEATS, MeleeBoard, MeleeOutcome};
use tiger_engine::valuation::StandardPipeline;
use tiger_engine::valuation::melee::MeleeValuation;
use tiger_engine::valuation::melee::Surveyed;
use tiger_engine::valuation::melee::appetite::{Appetite, REACH};
use tiger_engine::valuation::melee::attrition::Attrition;
use tiger_engine::valuation::melee::finish::MeleeFinish;
use tiger_engine::valuation::melee::finisher::Finisher;
use tiger_engine::valuation::melee::hunger::Hunger;
use tiger_engine::valuation::melee::larder::Larder;
use tiger_engine::valuation::melee::standing::{HeadDanger, Standing};
use tiger_engine::valuation::melee::territory::Territory;
use tiger_engine::valuation::melee::weights::DEFAULT_MELEE_PROFILE;
use tiger_engine::valuation::{Assessor, ValuationPipeline};

use support::{melee_spec, realize_melee, turn_state, turn_state_from_bodies};

fn melee(snakes: usize) -> MeleeBoard {
    ingest_melee(&turn_state(snakes, 0, &[])).expect("the test state is a melee")
}

fn continued(board: &MeleeBoard, by_seat: &[Heading]) -> MeleeBoard {
    let mut all = [Heading::North; MAX_SEATS];
    all[..by_seat.len()].copy_from_slice(by_seat);
    match board.advance(&all) {
        MeleeOutcome::Continues(next) => next,
        ended => panic!("the game unexpectedly ended: {ended:?}"),
    }
}

#[test]
fn attrition_counts_the_seats_already_gone() {
    assert_eq!(Attrition.assess(&Surveyed::new(&melee(4))), 0);
    assert_eq!(Attrition.assess(&Surveyed::new(&melee(3))), 1);

    let us: &[(i32, i32)] = &[(5, 5), (5, 4), (5, 3)];
    let wall: &[(i32, i32)] = &[(6, 5), (7, 5), (8, 5)];
    let far: &[(i32, i32)] = &[(1, 9), (1, 8), (1, 7)];
    let state = turn_state_from_bodies(&[us, wall, far], &[90, 90, 90], 0, &[]);
    let after_a_death = continued(
        &ingest_melee(&state).expect("melee"),
        &[Heading::North, Heading::West, Heading::North],
    );
    assert_eq!(Attrition.assess(&Surveyed::new(&after_a_death)), 2);
}

#[test]
fn a_melee_pipeline_weighs_its_terms_like_the_duel_one() {
    let pipeline = ValuationPipeline::empty().with(Attrition, 5_000);

    assert_eq!(pipeline.score(&Surveyed::new(&melee(3))), 5_000);
    assert_eq!(pipeline.worst_case_magnitude(), 15_000);
    let valuation = pipeline.assess(&Surveyed::new(&melee(3)));
    assert_eq!(valuation.ledger.entries()[0].name, "attrition");
    assert_eq!(valuation.ledger.entries()[0].raw, 1);
}

fn board_of(bodies: &[&[(i32, i32)]]) -> MeleeBoard {
    let health = vec![90; bodies.len()];
    ingest_melee(&turn_state_from_bodies(bodies, &health, 0, &[])).expect("melee")
}

const US: &[(i32, i32)] = &[(5, 5), (5, 4), (5, 3)];
const FAR: &[(i32, i32)] = &[(9, 0), (9, 1), (9, 2)];

#[test]
fn standing_compares_us_with_the_longest_living_rival() {
    let longer: &[(i32, i32)] = &[(1, 9), (1, 8), (1, 7), (1, 6), (1, 5)];
    let shorter: &[(i32, i32)] = &[(9, 9), (9, 8)];

    assert_eq!(
        Standing.assess(&Surveyed::new(&board_of(&[US, longer, shorter]))),
        -2
    );
    assert_eq!(
        Standing.assess(&Surveyed::new(&board_of(&[US, shorter, FAR]))),
        0
    );
    assert_eq!(
        Standing.assess(&Surveyed::new(&board_of(&[US, shorter, shorter_at(1, 0)]))),
        1
    );
}

fn shorter_at(x: i32, y: i32) -> &'static [(i32, i32)] {
    // A two-segment serpent coiled on one cell keeps the examples apart.
    Box::leak(vec![(x, y), (x, y)].into_boxed_slice())
}

#[test]
fn head_danger_counts_the_cells_we_share_with_stronger_and_weaker_heads() {
    // Two equal-length heads beside our reachable cells: (4,6) and (6,6) are
    // contested by them; (5,6) by both. Our cells: (4,5), (6,5), (5,6).
    let west: &[(i32, i32)] = &[(4, 7), (4, 8), (4, 9)];
    let east: &[(i32, i32)] = &[(6, 7), (6, 8), (6, 9)];
    let threatened = board_of(&[US, west, east]);
    // (4,6) is reached by west, (6,6) by east, (5,6) by neither; our (4,5), (6,5) by neither.
    // Cells we can enter: (4,5), (6,5), (5,6). West reaches (4,6),(3,7),(5,7); none of ours.
    assert_eq!(HeadDanger.assess(&Surveyed::new(&threatened)), 0);

    // A longer head right beside our next cells threatens two of them.
    let longer: &[(i32, i32)] = &[(6, 6), (7, 6), (8, 6), (9, 6)];
    let danger = board_of(&[US, longer, FAR]);
    // Longer reaches (5,6), (6,5), (6,7): two of our three cells.
    assert_eq!(HeadDanger.assess(&Surveyed::new(&danger)), -2);

    // A shorter head there instead is a chance on the same two cells.
    let shorter: &[(i32, i32)] = &[(6, 6), (7, 6)];
    let chance = board_of(&[US, shorter, FAR]);
    assert_eq!(HeadDanger.assess(&Surveyed::new(&chance)), 2);

    // Both a longer and a shorter head reaching (5,6): the danger wins the cell.
    let longer_west: &[(i32, i32)] = &[(4, 6), (3, 6), (2, 6), (1, 6)];
    let mixed = board_of(&[US, longer_west, shorter]);
    // Longer west reaches (5,6),(4,5); shorter east reaches (5,6),(6,5): -2 + 1.
    assert_eq!(HeadDanger.assess(&Surveyed::new(&mixed)), -1);
}

#[test]
fn hunger_is_our_pressure_alone_and_grows_as_food_slips_away() {
    let us_far: &[(i32, i32)] = &[(5, 5), (5, 4), (5, 3)];
    let comfortable = ingest_melee(&turn_state_from_bodies(
        &[us_far, FAR, shorter_at(0, 10)],
        &[90, 90, 90],
        0,
        &[(5, 6)],
    ))
    .unwrap();
    let starving = ingest_melee(&turn_state_from_bodies(
        &[us_far, FAR, shorter_at(0, 10)],
        &[3, 90, 90],
        0,
        &[(5, 10)],
    ))
    .unwrap();
    let no_food = ingest_melee(&turn_state_from_bodies(
        &[us_far, FAR, shorter_at(0, 10)],
        &[30, 1, 1],
        0,
        &[],
    ))
    .unwrap();

    assert_eq!(Hunger.assess(&Surveyed::new(&comfortable)), 0);
    // (5,10) is 5 from us and 5 from the shorter seat, so ours; with health 3
    // the margin is -2 and the pressure 25 + 2 = 27.
    assert_eq!(Hunger.assess(&Surveyed::new(&starving)), -27);
    // No pellet: distance 40 assumed, margin -10, pressure 35 (the rivals' hunger counts for nothing).
    assert_eq!(Hunger.assess(&Surveyed::new(&no_food)), -35);
}

#[test]
fn the_composed_valuation_uses_the_melee_terms_or_the_duel_terms_by_seat_count() {
    let valuation = MeleeValuation::standard();
    let three = melee(3);
    let two = MeleeBoard::try_new(
        &[
            *three.serpent(tiger_engine::arena::melee::Seat::US),
            *three.serpent(tiger_engine::arena::melee::Seat::ALL[1]),
        ],
        three.pellets(),
    )
    .unwrap();

    let duel_score = StandardPipeline::standard().score(&two.as_duel());
    assert_eq!(
        valuation.score(&two),
        duel_score + 2 * DEFAULT_MELEE_PROFILE.attrition_seat
    );
    assert_ne!(valuation.score(&three), 0);
    assert_eq!(
        valuation.score(&three),
        DEFAULT_MELEE_PROFILE.attrition_seat
            + 100 * (Territory.assess(&Surveyed::new(&three)))
            + 200 * Hunger.assess(&Surveyed::new(&three))
            + 250 * Standing.assess(&Surveyed::new(&three))
            + 300 * HeadDanger.assess(&Surveyed::new(&three))
    );
}

#[test]
fn the_standard_melee_valuation_stays_inside_the_finish_limits() {
    let worst = MeleeValuation::standard().worst_case_magnitude();
    let limit = MeleeFinish::new(&DEFAULT_MELEE_PROFILE).finite_limit();

    assert!(worst > 0);
    assert!(
        worst < limit,
        "worst case {worst} must stay below the finite limit {limit}"
    );
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn the_melee_score_is_deterministic_and_bounded(spec in melee_spec()) {
        let board = ingest_melee(&realize_melee(&spec)).unwrap();
        let valuation = MeleeValuation::standard();
        let score = valuation.score(&board);
        prop_assert_eq!(score, valuation.score(&board));
        prop_assert!(score.abs() <= valuation.worst_case_magnitude());
    }
}

#[test]
fn appetite_pulls_toward_the_nearest_pellet_we_reach_first_whatever_our_health() {
    let us: &[(i32, i32)] = &[(5, 5), (5, 4), (5, 3)];
    // A pellet three steps up, ours by distance; health is full, so hunger says nothing.
    let owned = ingest_melee(&turn_state_from_bodies(
        &[us, FAR, shorter_at(0, 10)],
        &[100, 90, 90],
        0,
        &[(5, 8)],
    ))
    .unwrap();
    // The only pellet sits beside the far rival's head: theirs, no pull.
    let theirs = ingest_melee(&turn_state_from_bodies(
        &[us, FAR, shorter_at(0, 10)],
        &[100, 90, 90],
        0,
        &[(10, 0)],
    ))
    .unwrap();
    let none = ingest_melee(&turn_state_from_bodies(
        &[us, FAR, shorter_at(0, 10)],
        &[100, 90, 90],
        0,
        &[],
    ))
    .unwrap();

    assert_eq!(REACH, 12);
    assert_eq!(Appetite.assess(&Surveyed::new(&owned)), REACH - 3);
    assert_eq!(Appetite.assess(&Surveyed::new(&theirs)), 0);
    assert_eq!(Appetite.assess(&Surveyed::new(&none)), 0);
    assert_eq!(
        Hunger.assess(&Surveyed::new(&owned)),
        0,
        "hunger is silent at full health; appetite is not"
    );
}

#[test]
#[allow(clippy::assertions_on_constants)]
fn eating_always_beats_hovering_beside_the_food() {
    // The whole appetite range must be worth less than one segment of standing.
    // Eating a pellet loses its larder value and the whole appetite range at most,
    // and gains one segment: the segment must be worth more.
    assert!(
        DEFAULT_MELEE_PROFILE.appetite_step * REACH + DEFAULT_MELEE_PROFILE.larder_pellet
            < DEFAULT_MELEE_PROFILE.standing_segment,
        "appetite {} x {} plus a larder pellet {} must stay below a segment {}",
        DEFAULT_MELEE_PROFILE.appetite_step,
        REACH,
        DEFAULT_MELEE_PROFILE.larder_pellet,
        DEFAULT_MELEE_PROFILE.standing_segment
    );
}

#[test]
fn the_larder_counts_the_pellets_we_reach_before_every_rival() {
    let us: &[(i32, i32)] = &[(5, 5), (5, 4), (5, 3)];
    // Two pellets beside us (ours), one beside the far rival, one at (2,8) the
    // short rival at (0,10) reaches first: two in the larder.
    let board = ingest_melee(&turn_state_from_bodies(
        &[us, FAR, shorter_at(0, 10)],
        &[90, 90, 90],
        0,
        &[(5, 7), (4, 5), (9, 3), (2, 8)],
    ))
    .unwrap();

    assert_eq!(Larder.assess(&Surveyed::new(&board)), 2);
}

#[test]
fn the_finisher_counts_the_exits_and_territory_denied_to_a_shorter_rival_in_contact() {
    // We are five long; a three-long rival two cells away sits in a corner with
    // one exit and little territory; another shorter rival is far away.
    let us: &[(i32, i32)] = &[(2, 1), (3, 1), (4, 1), (5, 1), (6, 1)];
    let cornered: &[(i32, i32)] = &[(0, 0), (0, 1), (0, 2)];
    let far: &[(i32, i32)] = &[(10, 10), (10, 9), (10, 8)];
    let board = ingest_melee(&turn_state_from_bodies(
        &[us, cornered, far],
        &[90; 3],
        0,
        &[],
    ))
    .unwrap();
    let raw = Finisher.assess(&Surveyed::new(&board));

    // The cornered head at (0,0) has one exit, (1,0): two exits taken; its
    // territory is small, so the term is positive and bounded.
    assert!((2..=3 + 8).contains(&raw), "{raw}");
    // A longer rival in contact contributes nothing.
    let longer: &[(i32, i32)] = &[(0, 0), (0, 1), (0, 2), (0, 3), (0, 4), (0, 5)];
    let board = ingest_melee(&turn_state_from_bodies(
        &[us, longer, far],
        &[90; 3],
        0,
        &[],
    ))
    .unwrap();
    assert_eq!(Finisher.assess(&Surveyed::new(&board)), 0);
}

#[test]
fn the_melee_ledger_adds_up_to_the_score_it_explains() {
    // Growth iteration 18: the melee could be scored but not read. A ledger
    // that did not add up to the score would be worse than none at all.
    let us: &[(i32, i32)] = &[(5, 5), (5, 4), (5, 3)];
    let rival: &[(i32, i32)] = &[(1, 1), (1, 2), (1, 3)];
    let third: &[(i32, i32)] = &[(9, 9), (9, 8), (9, 7)];
    let state = turn_state_from_bodies(&[us, rival, third], &[90, 90, 90], 0, &[(5, 8)]);
    let board = ingest_melee(&state).expect("a melee converts");
    let valuation = MeleeValuation::standard();

    let assessed = valuation.assess(&board).expect("three seats are alive");
    assert_eq!(assessed.score, valuation.score(&board));
    assert_eq!(
        assessed
            .ledger
            .entries()
            .iter()
            .map(|entry| entry.contribution())
            .sum::<i32>(),
        assessed.score,
        "the entries are the score, term by term"
    );
    assert!(
        assessed
            .ledger
            .entries()
            .iter()
            .any(|entry| entry.name == "appetite"),
        "every term the pipeline scores is named in the ledger"
    );
}
