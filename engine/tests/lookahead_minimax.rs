mod support;

use proptest::prelude::*;
use tiger_engine::arena::duel::{Advance, DuelBoard, Verdict};
use tiger_engine::arena::heading::Heading;
use tiger_engine::arena::ingest::ingest;
use tiger_engine::lookahead::minimax::Searcher;
use tiger_engine::valuation::finish::Finish;
use tiger_engine::valuation::leverage::LengthAdvantage;
use tiger_engine::valuation::weights::DEFAULT_PROFILE;
use tiger_engine::valuation::{AssessorSet, ValuationPipeline};

use support::{realize, state_spec, turn_state, turn_state_from_bodies};

fn finish() -> Finish {
    Finish::new(&DEFAULT_PROFILE)
}

/// Plain exhaustive minimax over the same rules, with no pruning and no shared
/// code with the searcher beyond the kernel and the evaluators.
struct Reference<'p, S> {
    pipeline: &'p ValuationPipeline<S>,
    finish: Finish,
    advances: std::cell::Cell<u64>,
}

impl<'p, S: AssessorSet> Reference<'p, S> {
    fn new(pipeline: &'p ValuationPipeline<S>) -> Self {
        Self {
            pipeline,
            finish: finish(),
            advances: std::cell::Cell::new(0),
        }
    }

    /// The value of answering `ours` from `board`: the worst case over replies.
    fn value_of_heading(&self, board: &DuelBoard, ours: Heading, depth: u16, ply: u16) -> i32 {
        Heading::ALL
            .into_iter()
            .map(|theirs| {
                self.advances.set(self.advances.get() + 1);
                match board.advance(ours, theirs) {
                    Advance::Over(verdict) => self.finish.score(verdict, ply + 1),
                    Advance::Continues(next) if depth == 1 => self.pipeline.score(&next),
                    Advance::Continues(next) => self.value(&next, depth - 1, ply + 1),
                }
            })
            .min()
            .expect("four replies")
    }

    fn value(&self, board: &DuelBoard, depth: u16, ply: u16) -> i32 {
        Heading::ALL
            .into_iter()
            .map(|ours| self.value_of_heading(board, ours, depth, ply))
            .max()
            .expect("four headings")
    }

    fn root_values(&self, board: &DuelBoard, depth: u16) -> [i32; 4] {
        Heading::ALL.map(|ours| self.value_of_heading(board, ours, depth, 0))
    }
}

fn first_best(values: [i32; 4]) -> Heading {
    let best = *values.iter().max().expect("four values");
    Heading::ALL[values
        .iter()
        .position(|&v| v == best)
        .expect("the max is present")]
}

fn length_pipeline() -> ValuationPipeline<((), tiger_engine::valuation::Weighted<LengthAdvantage>)>
{
    ValuationPipeline::empty().with(LengthAdvantage, 250)
}

fn start() -> DuelBoard {
    ingest(&turn_state(2, 0, &[(10, 10)])).expect("a duel converts")
}

#[test]
fn one_ply_matches_the_exhaustive_value_on_the_standard_start() {
    let pipeline = ValuationPipeline::standard();
    let reference = Reference::new(&pipeline);
    let board = start();

    let report = Searcher::new(&pipeline, finish()).search_fixed(&board, 1);

    let values = reference.root_values(&board, 1);
    assert_eq!(report.principal_score, Some(*values.iter().max().unwrap()));
    assert_eq!(report.best, Some(first_best(values)));
    assert_eq!(report.completed_depth, 1);
}

#[test]
fn three_plies_match_the_exhaustive_value_on_the_standard_start() {
    let pipeline = ValuationPipeline::standard();
    let reference = Reference::new(&pipeline);
    let board = start();

    let report = Searcher::new(&pipeline, finish()).search_fixed(&board, 3);

    let values = reference.root_values(&board, 3);
    assert_eq!(report.principal_score, Some(*values.iter().max().unwrap()));
    assert_eq!(report.best, Some(first_best(values)));
    assert_eq!(report.completed_depth, 3);
}

#[test]
fn pruning_visits_strictly_fewer_positions_than_the_exhaustive_search() {
    let pipeline = ValuationPipeline::standard();
    let reference = Reference::new(&pipeline);
    let board = start();

    let report = Searcher::new(&pipeline, finish()).search_fixed(&board, 3);
    reference.root_values(&board, 3);

    assert!(report.nodes_explored > 0);
    assert!(
        report.nodes_explored < reference.advances.get(),
        "{} nodes vs {} exhaustive",
        report.nodes_explored,
        reference.advances.get()
    );
}

#[test]
fn a_forced_win_is_scored_as_a_win_one_ply_from_the_root() {
    // They have one health left, so whatever they do they starve after this turn;
    // our snake is safe in the open, so any surviving move wins at ply 1.
    let us: &[(i32, i32)] = &[(5, 5), (5, 4), (5, 3)];
    let them: &[(i32, i32)] = &[(2, 2), (2, 1), (2, 0)];
    let board = ingest(&turn_state_from_bodies(&[us, them], &[90, 1], 0, &[])).expect("a duel");
    let pipeline = ValuationPipeline::standard();

    let report = Searcher::new(&pipeline, finish()).search_fixed(&board, 2);

    assert_eq!(
        report.principal_score,
        Some(DEFAULT_PROFILE.win_score - DEFAULT_PROFILE.ply_penalty)
    );
    assert_eq!(
        report.best,
        Some(Heading::North),
        "the first surviving heading in order"
    );
}

#[test]
fn a_depth_of_zero_is_a_programming_error() {
    let pipeline = ValuationPipeline::standard();

    let result = std::panic::catch_unwind(|| {
        Searcher::new(&pipeline, finish()).search_fixed(&start(), 0);
    });

    assert!(result.is_err());
}

fn certain_loss(board: &DuelBoard, ours: Heading) -> bool {
    Heading::ALL.into_iter().all(|theirs| {
        matches!(
            board.advance(ours, theirs),
            Advance::Over(Verdict::TheyOnly)
        )
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(80))]

    #[test]
    fn alpha_beta_equals_exhaustive_minimax_with_the_standard_pipeline(
        spec in state_spec(),
        depth in 1u16..=2,
    ) {
        let board = ingest(&realize(&spec)).expect("a duel converts");
        let pipeline = ValuationPipeline::standard();
        let reference = Reference::new(&pipeline);

        let report = Searcher::new(&pipeline, finish()).search_fixed(&board, depth);

        let values = reference.root_values(&board, depth);
        prop_assert_eq!(report.principal_score, Some(*values.iter().max().unwrap()));
        prop_assert_eq!(report.best, Some(first_best(values)));
        prop_assert!(report.nodes_explored <= reference.advances.get());
    }

    #[test]
    fn alpha_beta_equals_exhaustive_minimax_to_three_plies_with_a_cheap_evaluator(
        spec in state_spec(),
        depth in 1u16..=3,
    ) {
        let board = ingest(&realize(&spec)).expect("a duel converts");
        let pipeline = length_pipeline();
        let reference = Reference::new(&pipeline);

        let report = Searcher::new(&pipeline, finish()).search_fixed(&board, depth);

        let values = reference.root_values(&board, depth);
        prop_assert_eq!(report.principal_score, Some(*values.iter().max().unwrap()));
        prop_assert_eq!(report.best, Some(first_best(values)));
    }

    #[test]
    fn the_chosen_heading_is_never_a_certain_loss_when_another_heading_exists(
        spec in state_spec(),
        depth in 1u16..=2,
    ) {
        let board = ingest(&realize(&spec)).expect("a duel converts");
        let pipeline = ValuationPipeline::standard();
        prop_assume!(Heading::ALL.into_iter().any(|h| !certain_loss(&board, h)));

        let report = Searcher::new(&pipeline, finish()).search_fixed(&board, depth);

        prop_assert!(!certain_loss(&board, report.best.expect("a heading")));
    }
}
