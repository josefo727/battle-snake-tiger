mod support;

use tiger_engine::arena::duel::{DuelBoard, Side};
use tiger_engine::arena::ingest::ingest;
use tiger_engine::valuation::finish::Finish;
use tiger_engine::valuation::weights::{DEFAULT_PROFILE, WeightSheet};
use tiger_engine::valuation::{Assessor, Ledger, LedgerEntry, LedgerSink, ValuationPipeline};

use proptest::prelude::*;
use support::{realize, state_spec, turn_state};

struct Constant(i32);

impl Assessor for Constant {
    const NAME: &'static str = "constant";
    const MAX_RAW: i32 = 1_000;

    fn assess(&self, _board: &DuelBoard) -> i32 {
        self.0
    }
}

struct OurLength;

impl Assessor for OurLength {
    const NAME: &'static str = "our_length";
    const MAX_RAW: i32 = 121;

    fn assess(&self, board: &DuelBoard) -> i32 {
        i32::from(board.serpent(Side::Us).length())
    }
}

fn open_board() -> DuelBoard {
    ingest(&turn_state(2, 0, &[])).expect("a duel converts")
}

fn entry(name: &'static str, raw: i32, weight: i32) -> LedgerEntry {
    LedgerEntry { name, raw, weight }
}

#[test]
fn an_empty_pipeline_scores_zero_with_an_empty_ledger() {
    let pipeline = ValuationPipeline::empty();

    let valuation = pipeline.assess(&open_board());

    assert_eq!(pipeline.score(&open_board()), 0);
    assert_eq!(valuation.score, 0);
    assert!(valuation.ledger.entries().is_empty());
    assert_eq!(valuation.ledger.total(), 0);
}

#[test]
fn the_score_is_the_weighted_sum_and_the_ledger_lists_terms_in_order() {
    let pipeline = ValuationPipeline::empty()
        .with(Constant(3), 10)
        .with(Constant(-2), 5);

    let valuation = pipeline.assess(&open_board());

    assert_eq!(valuation.score, 20);
    assert_eq!(pipeline.score(&open_board()), 20);
    assert_eq!(
        valuation.ledger.entries(),
        &[entry("constant", 3, 10), entry("constant", -2, 5)]
    );
    assert_eq!(valuation.ledger.total(), valuation.score);
}

#[test]
fn a_zero_weight_term_contributes_nothing_but_is_still_reported() {
    let pipeline = ValuationPipeline::empty().with(Constant(7), 0);

    let valuation = pipeline.assess(&open_board());

    assert_eq!(valuation.score, 0);
    assert_eq!(valuation.ledger.entries(), &[entry("constant", 7, 0)]);
}

#[test]
fn assessors_receive_the_position_being_scored() {
    let pipeline = ValuationPipeline::empty().with(OurLength, 4);

    assert_eq!(pipeline.score(&open_board()), 12);
}

#[test]
fn a_ledger_records_entries_in_order_up_to_its_capacity_and_totals_them() {
    let mut ledger = Ledger::new();
    for raw in 1..=8 {
        ledger.record(entry("term", raw, 2));
    }

    assert_eq!(ledger.entries().len(), 8);
    assert_eq!(ledger.entries()[0], entry("term", 1, 2));
    assert_eq!(ledger.entries()[7], entry("term", 8, 2));
    assert_eq!(ledger.total(), 72);
}

#[test]
fn the_default_profile_is_a_constant_that_experiments_can_override_by_name() {
    const PROFILE: WeightSheet = DEFAULT_PROFILE;
    let profile = PROFILE;
    let experiment = WeightSheet {
        territory_cell: 250,
        ..profile
    };

    assert!(profile.win_score > 0);
    assert!(profile.ply_penalty > 0);
    assert_eq!(experiment.territory_cell, 250);
    assert_eq!(experiment.win_score, profile.win_score);
}

#[test]
fn the_standard_pipeline_registers_the_five_terms_with_the_default_weights_in_order() {
    let valuation = ValuationPipeline::standard().assess(&open_board());

    let listed: Vec<(&str, i32)> = valuation
        .ledger
        .entries()
        .iter()
        .map(|entry| (entry.name, entry.weight))
        .collect();
    assert_eq!(
        listed,
        vec![
            ("dominion", DEFAULT_PROFILE.territory_cell),
            ("sustenance", DEFAULT_PROFILE.hunger_urgency),
            ("length_advantage", DEFAULT_PROFILE.length_advantage),
            ("head_pressure", DEFAULT_PROFILE.head_pressure),
            ("enclosure", DEFAULT_PROFILE.enclosure_turn),
        ]
    );
    assert_eq!(valuation.ledger.total(), valuation.score);
}

#[test]
fn the_standard_pipelines_worst_case_is_the_sum_of_weight_times_bound() {
    // 121*100 (territory) + 60*200 (hunger) + 120*250 (length) + 4*150 (head
    // pressure) + 121*120 (enclosure).
    let worst = ValuationPipeline::standard().worst_case_magnitude();

    assert_eq!(worst, 12_100 + 12_000 + 30_000 + 600 + 14_520);
    assert!(
        worst < Finish::new(&DEFAULT_PROFILE).finite_limit(),
        "a positional score must never look like a forced win or loss"
    );
}

fn swapped(board: &DuelBoard) -> DuelBoard {
    DuelBoard::try_new(
        *board.serpent(Side::Them),
        *board.serpent(Side::Us),
        board.pellets(),
    )
    .expect("swapping two valid serpents stays valid")
}

proptest! {
    #[test]
    fn swapping_the_serpents_negates_the_standard_score(spec in state_spec()) {
        let b = ingest(&realize(&spec)).expect("a duel converts");
        let pipeline = ValuationPipeline::standard();

        prop_assert_eq!(pipeline.score(&swapped(&b)), -pipeline.score(&b));
    }

    #[test]
    fn the_standard_score_is_deterministic_and_stays_below_the_finite_limit(spec in state_spec()) {
        let b = ingest(&realize(&spec)).expect("a duel converts");
        let pipeline = ValuationPipeline::standard();
        let limit = Finish::new(&DEFAULT_PROFILE).finite_limit();

        let first = pipeline.score(&b);

        prop_assert_eq!(first, pipeline.score(&b));
        prop_assert_eq!(first, pipeline.assess(&b).score);
        prop_assert!(first.abs() < limit);
    }
}
