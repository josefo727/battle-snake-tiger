mod support;

use proptest::prelude::*;
use tiger_engine::arena::duel::{DuelBoard, Side};
use tiger_engine::arena::heading::Heading::{self, East, North, South, West};
use tiger_engine::arena::ingest::ingest;
use tiger_engine::lookahead::minimax::Searcher;
use tiger_engine::lookahead::ordering::{HeadingOrder, LearnedOrder, NaturalOrder};
use tiger_engine::valuation::finish::Finish;
use tiger_engine::valuation::leverage::LengthAdvantage;
use tiger_engine::valuation::weights::DEFAULT_PROFILE;
use tiger_engine::valuation::{AssessorSet, ValuationPipeline};

use support::{realize, state_spec, turn_state, turn_state_from_bodies};

const NATURAL: [Heading; 4] = Heading::ALL;

fn finish() -> Finish {
    Finish::new(&DEFAULT_PROFILE)
}

// ---- the learned order itself ----------------------------------------------

#[test]
fn an_untrained_order_is_the_fixed_heading_order() {
    let order = LearnedOrder::new();

    for side in [Side::Us, Side::Them] {
        for ply in [0, 1, 7] {
            assert_eq!(order.arrange(side, ply), NATURAL);
        }
    }
}

#[test]
fn the_previous_best_comes_first_at_the_root_for_our_side_only() {
    let mut order = LearnedOrder::new();
    order.note_root_best(South);

    assert_eq!(order.arrange(Side::Us, 0), [South, North, East, West]);
    assert_eq!(order.arrange(Side::Us, 1), NATURAL);
    assert_eq!(order.arrange(Side::Them, 0), NATURAL);
}

#[test]
fn killers_follow_the_previous_best_most_recent_first() {
    let mut order = LearnedOrder::new();
    order.note_root_best(South);
    order.note_cutoff(Side::Us, 0, West, 3);
    order.note_cutoff(Side::Us, 0, East, 3);

    assert_eq!(order.arrange(Side::Us, 0), [South, East, West, North]);
}

#[test]
fn only_two_killers_are_kept_per_ply_and_side() {
    let mut order = LearnedOrder::new();
    order.note_cutoff(Side::Them, 4, North, 2);
    order.note_cutoff(Side::Them, 4, East, 2);
    order.note_cutoff(Side::Them, 4, South, 2);

    let arranged = order.arrange(Side::Them, 4);

    assert_eq!(&arranged[..2], &[South, East]);
    assert_eq!(
        order.arrange(Side::Them, 5)[0],
        North,
        "other plies see history only"
    );
    assert_eq!(order.arrange(Side::Us, 4), NATURAL, "history is per side");
}

#[test]
fn killers_are_per_ply_and_per_side() {
    let mut order = LearnedOrder::new();
    order.note_cutoff(Side::Us, 3, West, 1);

    assert_eq!(order.arrange(Side::Us, 3)[0], West);
    assert_eq!(order.arrange(Side::Them, 3), NATURAL);
    assert_eq!(
        order.arrange(Side::Us, 2)[0],
        West,
        "history still ranks it first"
    );
}

#[test]
fn a_killer_equal_to_the_previous_best_is_not_repeated() {
    let mut order = LearnedOrder::new();
    order.note_root_best(East);
    order.note_cutoff(Side::Us, 0, East, 2);
    order.note_cutoff(Side::Us, 0, West, 2);

    assert_eq!(order.arrange(Side::Us, 0), [East, West, North, South]);
}

#[test]
fn history_ranks_the_remaining_headings_by_deeper_cutoffs() {
    let mut order = LearnedOrder::new();
    order.note_cutoff(Side::Us, 5, South, 3);
    order.note_cutoff(Side::Us, 6, East, 1);

    assert_eq!(order.arrange(Side::Us, 0), [South, East, North, West]);
}

#[test]
fn equal_history_falls_back_to_the_fixed_order() {
    let mut order = LearnedOrder::new();
    order.note_cutoff(Side::Us, 5, West, 2);
    order.note_cutoff(Side::Us, 6, East, 2);

    assert_eq!(order.arrange(Side::Us, 0), [East, West, North, South]);
}

#[test]
fn plies_beyond_the_table_are_clamped_not_a_crash() {
    let mut order = LearnedOrder::new();
    order.note_cutoff(Side::Us, u16::MAX, South, u16::MAX);

    assert_eq!(order.arrange(Side::Us, u16::MAX)[0], South);
}

#[test]
fn the_natural_order_ignores_everything_it_is_told() {
    let mut order = NaturalOrder;
    order.note_root_best(West);
    order.note_cutoff(Side::Us, 0, West, 4);

    assert_eq!(order.arrange(Side::Us, 0), NATURAL);
}

#[derive(Clone, Debug)]
enum Note {
    Root(usize),
    Cutoff(bool, u16, usize, u16),
}

fn note() -> impl Strategy<Value = Note> {
    prop_oneof![
        (0usize..4).prop_map(Note::Root),
        (any::<bool>(), 0u16..12, 0usize..4, 1u16..9)
            .prop_map(|(us, ply, heading, depth)| Note::Cutoff(us, ply, heading, depth)),
    ]
}

fn side(us: bool) -> Side {
    if us { Side::Us } else { Side::Them }
}

proptest! {
    #[test]
    fn every_arrangement_is_a_permutation_and_reproducible(
        notes in proptest::collection::vec(note(), 0..40),
    ) {
        let mut order = LearnedOrder::new();
        for n in &notes {
            match *n {
                Note::Root(h) => order.note_root_best(Heading::ALL[h]),
                Note::Cutoff(us, ply, h, depth) => {
                    order.note_cutoff(side(us), ply, Heading::ALL[h], depth);
                }
            }
        }
        let twin = order.clone();

        for us in [true, false] {
            for ply in 0..12 {
                let arranged = order.arrange(side(us), ply);
                let mut seen = [false; 4];
                for h in arranged {
                    seen[h.index()] = true;
                }
                prop_assert!(seen.iter().all(|&s| s), "{arranged:?}");
                prop_assert_eq!(arranged, twin.arrange(side(us), ply));
            }
        }
    }
}

// ---- the order changes the work, never the answer --------------------------

/// The same permutation at every layer, for any test that needs a hostile order.
struct Fixed([Heading; 4]);

impl HeadingOrder for Fixed {
    fn arrange(&self, _side: Side, _ply: u16) -> [Heading; 4] {
        self.0
    }
    fn note_cutoff(&mut self, _side: Side, _ply: u16, _heading: Heading, _depth: u16) {}
    fn note_root_best(&mut self, _heading: Heading) {}
}

fn length_pipeline() -> ValuationPipeline<((), tiger_engine::valuation::Weighted<LengthAdvantage>)>
{
    ValuationPipeline::empty().with(LengthAdvantage, 250)
}

fn suite() -> Vec<DuelBoard> {
    let states = [
        turn_state(2, 0, &[(3, 7), (8, 2), (6, 8)]),
        turn_state_from_bodies(
            &[&[(4, 5), (3, 5), (2, 5)], &[(7, 5), (8, 5), (9, 5)]],
            &[90, 90],
            0,
            &[(5, 8), (6, 2)],
        ),
        turn_state_from_bodies(
            &[
                &[(5, 5), (5, 4), (5, 3), (5, 2), (4, 2)],
                &[(5, 7), (6, 7), (7, 7), (7, 6)],
            ],
            &[70, 60],
            0,
            &[],
        ),
        turn_state_from_bodies(
            &[&[(1, 1), (1, 2), (1, 3)], &[(9, 9), (9, 8), (9, 7)]],
            &[6, 80],
            0,
            &[(4, 4)],
        ),
        turn_state_from_bodies(
            &[&[(0, 5), (1, 5), (2, 5)], &[(2, 7), (2, 6), (3, 6)]],
            &[95, 50],
            0,
            &[(0, 9)],
        ),
        turn_state_from_bodies(
            &[
                &[(5, 5), (6, 5), (7, 5), (7, 6), (6, 6)],
                &[(5, 6), (4, 6), (3, 6), (3, 5)],
            ],
            &[80, 80],
            0,
            &[(9, 1)],
        ),
    ];
    states
        .iter()
        .map(|s| ingest(s).expect("the suite holds duels"))
        .collect()
}

#[test]
fn learned_ordering_visits_strictly_fewer_nodes_on_a_fixed_suite_and_agrees_on_the_answer() {
    const DEPTH: u16 = 5;
    let pipeline = ValuationPipeline::standard();
    let mut natural_total = 0;
    let mut learned_total = 0;

    for board in suite() {
        let mut natural = Searcher::with_order(&pipeline, finish(), NaturalOrder);
        let mut learned = Searcher::new(&pipeline, finish());
        // Deepen both the way the driver will, so the learned order has history.
        for depth in 1..=DEPTH {
            let plain = natural.search_fixed(&board, depth);
            let ordered = learned.search_fixed(&board, depth);
            assert_eq!(
                ordered.principal_score, plain.principal_score,
                "depth {depth}"
            );
            assert_eq!(ordered.best, plain.best, "depth {depth}");
            if depth == DEPTH {
                natural_total += plain.nodes_explored;
                learned_total += ordered.nodes_explored;
            }
        }
    }

    // Measured in release on this suite: 8,592 vs 33,357 nodes at depth 5. Demanding
    // half keeps a real margin while still catching a regression to no ordering.
    assert!(
        learned_total * 2 < natural_total,
        "learned {learned_total} nodes vs natural {natural_total}"
    );
}

fn agrees_under_any_layer_order<S: AssessorSet>(
    pipeline: &ValuationPipeline<S>,
    board: &DuelBoard,
    permutation: [Heading; 4],
    depth: u16,
) -> Result<(), TestCaseError> {
    let plain = Searcher::with_order(pipeline, finish(), NaturalOrder).search_fixed(board, depth);
    let shuffled =
        Searcher::with_order(pipeline, finish(), Fixed(permutation)).search_fixed(board, depth);

    prop_assert_eq!(shuffled.principal_score, plain.principal_score);
    prop_assert_eq!(
        shuffled.best,
        plain.best,
        "ties must resolve by the fixed order"
    );
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(80))]

    #[test]
    fn any_layer_order_gives_the_same_value_and_heading_as_the_fixed_order(
        spec in state_spec(),
        permutation in Just(Heading::ALL).prop_shuffle(),
        depth in 1u16..=3,
    ) {
        let board = ingest(&realize(&spec)).expect("a duel converts");

        agrees_under_any_layer_order(&length_pipeline(), &board, permutation, depth)?;
    }

    #[test]
    fn any_layer_order_agrees_with_the_standard_pipeline_too(
        spec in state_spec(),
        permutation in Just(Heading::ALL).prop_shuffle(),
        depth in 1u16..=2,
    ) {
        let board = ingest(&realize(&spec)).expect("a duel converts");

        agrees_under_any_layer_order(&ValuationPipeline::standard(), &board, permutation, depth)?;
    }

    #[test]
    fn a_learning_searcher_agrees_with_the_fixed_order_across_deepening(
        spec in state_spec(),
    ) {
        let board = ingest(&realize(&spec)).expect("a duel converts");
        let pipeline = length_pipeline();
        let mut learned = Searcher::new(&pipeline, finish());
        let mut plain = Searcher::with_order(&pipeline, finish(), NaturalOrder);

        for depth in 1..=3 {
            let a = learned.search_fixed(&board, depth);
            let b = plain.search_fixed(&board, depth);
            prop_assert_eq!(a.principal_score, b.principal_score);
            prop_assert_eq!(a.best, b.best);
        }
    }
}
