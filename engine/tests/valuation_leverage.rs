mod support;

use proptest::prelude::*;
use tiger_engine::arena::duel::{DuelBoard, Side};
use tiger_engine::arena::ingest::ingest;
use tiger_engine::valuation::leverage::{HeadPressure, LengthAdvantage};
use tiger_engine::valuation::{Assessor, ValuationPipeline};

use support::{realize, state_spec, turn_state_from_bodies};

fn board(us: &[(i32, i32)], them: &[(i32, i32)]) -> DuelBoard {
    ingest(&turn_state_from_bodies(&[us, them], &[90, 90], 0, &[])).expect("a duel converts")
}

fn swapped(board: &DuelBoard) -> DuelBoard {
    DuelBoard::try_new(
        *board.serpent(Side::Them),
        *board.serpent(Side::Us),
        board.pellets(),
    )
    .expect("swapping two valid serpents stays valid")
}

const SHORT: &[(i32, i32)] = &[(1, 1), (1, 0), (0, 0)];
const LONG: &[(i32, i32)] = &[(9, 9), (9, 8), (9, 7), (9, 6), (9, 5)];

#[test]
fn length_advantage_is_our_length_minus_theirs() {
    assert_eq!(LengthAdvantage.assess(&board(LONG, SHORT)), 2);
    assert_eq!(LengthAdvantage.assess(&board(SHORT, LONG)), -2);
    assert_eq!(LengthAdvantage.assess(&board(SHORT, SHORT_ELSEWHERE)), 0);
}

const SHORT_ELSEWHERE: &[(i32, i32)] = &[(10, 1), (10, 0), (9, 0)];

#[test]
fn a_single_contested_cell_favours_the_longer_serpent() {
    // Heads at (4, 5) and (6, 5) can both step onto (5, 5) next turn.
    let ours_long = board(&[(4, 5), (3, 5), (2, 5), (1, 5)], &[(6, 5), (7, 5), (8, 5)]);
    let theirs_long = board(&[(4, 5), (3, 5), (2, 5)], &[(6, 5), (7, 5), (8, 5), (9, 5)]);
    let equal = board(&[(4, 5), (3, 5), (2, 5)], &[(6, 5), (7, 5), (8, 5)]);

    assert_eq!(HeadPressure.assess(&ours_long), 1);
    assert_eq!(HeadPressure.assess(&theirs_long), -1);
    assert_eq!(HeadPressure.assess(&equal), 0);
}

#[test]
fn diagonal_heads_contest_two_cells() {
    let b = board(&[(4, 4), (3, 4), (2, 4), (1, 4)], &[(5, 5), (6, 5), (7, 5)]);

    assert_eq!(HeadPressure.assess(&b), 2);
}

#[test]
fn heads_that_cannot_meet_next_turn_have_no_pressure() {
    let b = board(
        &[(1, 1), (1, 0), (0, 0), (0, 1)],
        &[(9, 9), (9, 10), (10, 10)],
    );

    assert_eq!(HeadPressure.assess(&b), 0);
}

#[test]
fn a_vacating_tail_is_a_cell_both_heads_can_enter_but_a_stacked_tail_is_not() {
    // Our coil's tail (4, 5) is beside our head (5, 5) and beside their head
    // (3, 5). It frees this turn, so both can enter it; with a second copy stacked
    // on it, it stays occupied and nobody can.
    let coil: &[(i32, i32)] = &[(5, 5), (5, 4), (4, 4), (4, 5)];
    let stacked_coil: &[(i32, i32)] = &[(5, 5), (5, 4), (4, 4), (4, 5), (4, 5)];
    let them: &[(i32, i32)] = &[(3, 5), (2, 5), (1, 5)];

    assert_eq!(HeadPressure.assess(&board(coil, them)), 1);
    assert_eq!(HeadPressure.assess(&board(stacked_coil, them)), 0);
}

#[test]
fn both_terms_weight_through_the_pipeline() {
    let b = board(&[(4, 5), (3, 5), (2, 5), (1, 5)], &[(6, 5), (7, 5), (8, 5)]);
    let pipeline = ValuationPipeline::empty()
        .with(LengthAdvantage, 250)
        .with(HeadPressure, 150);

    assert_eq!(pipeline.score(&b), 250 + 150);
}

proptest! {
    #[test]
    fn swapping_the_serpents_negates_both_terms(spec in state_spec()) {
        let b = ingest(&realize(&spec)).expect("a duel converts");

        prop_assert_eq!(LengthAdvantage.assess(&swapped(&b)), -LengthAdvantage.assess(&b));
        prop_assert_eq!(HeadPressure.assess(&swapped(&b)), -HeadPressure.assess(&b));
    }
}

proptest! {
    // Equal lengths are a small slice of generated duels, so allow many rejections.
    #![proptest_config(ProptestConfig { cases: 200, max_global_rejects: 100_000, ..ProptestConfig::default() })]

    #[test]
    fn equal_lengths_never_produce_a_head_to_head_term(spec in state_spec()) {
        let b = ingest(&realize(&spec)).expect("a duel converts");
        prop_assume!(b.serpent(Side::Us).length() == b.serpent(Side::Them).length());

        prop_assert_eq!(HeadPressure.assess(&b), 0);
    }
}
