mod support;

use tiger_engine::arena::heading::Heading;
use tiger_engine::arena::ingest::ingest_melee;
use tiger_engine::arena::melee::{MAX_SEATS, MeleeBoard, MeleeOutcome};
use tiger_engine::valuation::melee::attrition::Attrition;
use tiger_engine::valuation::{Assessor, ValuationPipeline};

use support::{turn_state, turn_state_from_bodies};

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
    assert_eq!(Attrition.assess(&melee(4)), 0);
    assert_eq!(Attrition.assess(&melee(3)), 1);

    let us: &[(i32, i32)] = &[(5, 5), (5, 4), (5, 3)];
    let wall: &[(i32, i32)] = &[(6, 5), (7, 5), (8, 5)];
    let far: &[(i32, i32)] = &[(1, 9), (1, 8), (1, 7)];
    let state = turn_state_from_bodies(&[us, wall, far], &[90, 90, 90], 0, &[]);
    let after_a_death = continued(
        &ingest_melee(&state).expect("melee"),
        &[Heading::North, Heading::West, Heading::North],
    );
    assert_eq!(Attrition.assess(&after_a_death), 2);
}

#[test]
fn a_melee_pipeline_weighs_its_terms_like_the_duel_one() {
    let pipeline = ValuationPipeline::empty().with(Attrition, 5_000);

    assert_eq!(pipeline.score(&melee(3)), 5_000);
    assert_eq!(pipeline.worst_case_magnitude(), 15_000);
    let valuation = pipeline.assess(&melee(3));
    assert_eq!(valuation.ledger.entries()[0].name, "attrition");
    assert_eq!(valuation.ledger.entries()[0].raw, 1);
}
