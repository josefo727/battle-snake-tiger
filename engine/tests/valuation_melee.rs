mod support;

use tiger_engine::arena::heading::Heading;
use tiger_engine::arena::ingest::ingest_melee;
use tiger_engine::arena::melee::{MAX_SEATS, MeleeBoard, MeleeOutcome};
use tiger_engine::valuation::melee::attrition::Attrition;
use tiger_engine::valuation::melee::standing::{HeadDanger, Standing};
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

    assert_eq!(Standing.assess(&board_of(&[US, longer, shorter])), -2);
    assert_eq!(Standing.assess(&board_of(&[US, shorter, FAR])), 0);
    assert_eq!(
        Standing.assess(&board_of(&[US, shorter, shorter_at(1, 0)])),
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
    assert_eq!(HeadDanger.assess(&threatened), 0);

    // A longer head right beside our next cells threatens two of them.
    let longer: &[(i32, i32)] = &[(6, 6), (7, 6), (8, 6), (9, 6)];
    let danger = board_of(&[US, longer, FAR]);
    // Longer reaches (5,6), (6,5), (6,7): two of our three cells.
    assert_eq!(HeadDanger.assess(&danger), -2);

    // A shorter head there instead is a chance on the same two cells.
    let shorter: &[(i32, i32)] = &[(6, 6), (7, 6)];
    let chance = board_of(&[US, shorter, FAR]);
    assert_eq!(HeadDanger.assess(&chance), 2);

    // Both a longer and a shorter head reaching (5,6): the danger wins the cell.
    let longer_west: &[(i32, i32)] = &[(4, 6), (3, 6), (2, 6), (1, 6)];
    let mixed = board_of(&[US, longer_west, shorter]);
    // Longer west reaches (5,6),(4,5); shorter east reaches (5,6),(6,5): -2 + 1.
    assert_eq!(HeadDanger.assess(&mixed), -1);
}
