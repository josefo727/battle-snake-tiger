mod support;

use proptest::prelude::*;
use tiger_engine::arena::cellset::CellSet;
use tiger_engine::arena::heading::Heading;
use tiger_engine::arena::ingest::ingest_melee;
use tiger_engine::arena::melee::{MAX_SEATS, MeleeBoard, MeleeOutcome, Seat};
use tiger_engine::lookahead::paranoid::MeleeSearcher;
use tiger_engine::valuation::melee::MeleeValuation;
use tiger_engine::valuation::melee::finish::MeleeFinish;
use tiger_engine::valuation::melee::weights::DEFAULT_MELEE_PROFILE;

use support::{melee_spec, realize_melee, turn_state, turn_state_from_bodies};

fn finish() -> MeleeFinish {
    MeleeFinish::new(&DEFAULT_MELEE_PROFILE)
}

/// Every joint reply of the living opponents (seat order), as full move arrays
/// with our heading in seat 0.
fn joint_replies(board: &MeleeBoard, ours: Heading) -> Vec<[Heading; MAX_SEATS]> {
    let rivals: Vec<Seat> = board.seats().filter(|s| *s != Seat::US).collect();
    let total = 4usize.pow(rivals.len() as u32);
    (0..total)
        .map(|code| {
            let mut moves = [Heading::North; MAX_SEATS];
            moves[0] = ours;
            for (k, seat) in rivals.iter().enumerate() {
                moves[seat.index()] = Heading::ALL[(code >> (2 * k)) & 3];
            }
            moves
        })
        .collect()
}

/// Plain exhaustive paranoid minimax: our max over the min of every joint
/// reply, with no pruning and nothing shared with the searcher but the kernel
/// and the valuation.
struct Reference<'v> {
    valuation: &'v MeleeValuation,
    finish: MeleeFinish,
    /// Below the root, drop the opponent replies that kill their own serpent
    /// outright (unless a seat has no other), like the searcher's pruning.
    filtered: bool,
}

/// Cells a head can step into without dying on the spot: free cells and the
/// tails that vacate this turn.
fn enterable(board: &MeleeBoard) -> CellSet {
    board
        .seats()
        .fold(board.occupied().complement(), |cells, seat| {
            board
                .serpent(seat)
                .cell_released_on_turn(1)
                .map_or(cells, |cell| cells.with(cell))
        })
}

fn is_self_preserving(board: &MeleeBoard, seat: Seat, heading: Heading) -> bool {
    heading
        .step(board.serpent(seat).head())
        .is_some_and(|cell| enterable(board).contains(cell))
}

impl Reference<'_> {
    fn allowed(&self, board: &MeleeBoard, moves: &[Heading; MAX_SEATS], ply: u16) -> bool {
        if !self.filtered || ply == 0 {
            return true;
        }
        board.seats().filter(|s| *s != Seat::US).all(|seat| {
            let has_safe = Heading::ALL
                .into_iter()
                .any(|h| is_self_preserving(board, seat, h));
            !has_safe || is_self_preserving(board, seat, moves[seat.index()])
        })
    }

    fn value_of_heading(&self, board: &MeleeBoard, ours: Heading, depth: u16, ply: u16) -> i32 {
        joint_replies(board, ours)
            .into_iter()
            .filter(|moves| self.allowed(board, moves, ply))
            .map(|moves| match board.advance(&moves) {
                MeleeOutcome::Continues(next) if depth == 1 => self.valuation.score(&next),
                MeleeOutcome::Continues(next) => self.value(&next, depth - 1, ply + 1),
                ended => self.finish.score(&ended, ply + 1),
            })
            .min()
            .expect("at least one reply")
    }

    fn value(&self, board: &MeleeBoard, depth: u16, ply: u16) -> i32 {
        Heading::ALL
            .into_iter()
            .map(|ours| self.value_of_heading(board, ours, depth, ply))
            .max()
            .expect("four headings")
    }

    fn root_values(&self, board: &MeleeBoard, depth: u16) -> [i32; 4] {
        Heading::ALL.map(|ours| self.value_of_heading(board, ours, depth, 0))
    }
}

fn first_best(values: [i32; 4]) -> Heading {
    let best = *values.iter().max().expect("four values");
    Heading::ALL[values.iter().position(|&v| v == best).expect("present")]
}

fn assert_matches_reference(board: &MeleeBoard, depth: u16) {
    let valuation = MeleeValuation::standard();
    let reference = Reference {
        valuation: &valuation,
        finish: finish(),
        filtered: true,
    };
    let values = reference.root_values(board, depth);
    let mut searcher = MeleeSearcher::new(&valuation, finish());

    let report = searcher.search_fixed(board, depth);

    assert_eq!(report.completed_depth, depth);
    assert_eq!(
        report.principal_score,
        Some(*values.iter().max().unwrap()),
        "depth {depth}: values {values:?}"
    );
    assert_eq!(
        report.best,
        Some(first_best(values)),
        "depth {depth}: values {values:?}"
    );
}

#[test]
fn the_opening_four_snake_position_is_searched_like_the_reference_at_depth_one() {
    let board = ingest_melee(&turn_state(4, 0, &[(5, 6)])).expect("melee");

    assert_matches_reference(&board, 1);
    let valuation = MeleeValuation::standard();
    let mut searcher = MeleeSearcher::new(&valuation, finish());
    let report = searcher.search_fixed(&board, 1);
    // One node per joint move played; pruning keeps it well under the 4 * 64 of
    // the exhaustive reference.
    assert!(
        report.nodes_explored > 0 && report.nodes_explored < 4 * 64,
        "{}",
        report.nodes_explored
    );
    assert_eq!(searcher.nodes_visited(), report.nodes_explored);
}

#[test]
fn a_three_snake_position_is_searched_like_the_reference_at_depth_two() {
    let board = ingest_melee(&turn_state(3, 1, &[(2, 3), (8, 8)])).expect("melee");

    assert_matches_reference(&board, 2);
}

#[test]
fn a_certain_win_and_a_certain_loss_are_decisive_scores() {
    // We are longer and both rivals must step into our head's cell or a wall.
    let us: &[(i32, i32)] = &[(1, 0), (2, 0), (3, 0), (4, 0)];
    let cornered: &[(i32, i32)] = &[(0, 1), (0, 2), (0, 3)];
    let far: &[(i32, i32)] = &[(9, 9), (9, 8), (9, 7)];
    let state = turn_state_from_bodies(&[cornered, us, far], &[90, 90, 90], 0, &[]);
    let board = ingest_melee(&state).expect("melee");
    let valuation = MeleeValuation::standard();
    let mut searcher = MeleeSearcher::new(&valuation, finish());

    let report = searcher.search_fixed(&board, 1);

    // Cornered at (0,1) with the body below: north survives only if the longer
    // serpent does not take (0,1)... it can, so every line loses.
    let score = report.principal_score.expect("completed");
    assert!(searcher.is_decisive(score), "{score}");
    assert!(score < 0, "{score}");
    assert_matches_reference(&board, 1);
}

#[test]
fn pruning_the_opponents_suicides_below_the_root_saves_work_and_keeps_the_value() {
    let board = ingest_melee(&turn_state(4, 0, &[(5, 6)])).expect("melee");
    let valuation = MeleeValuation::standard();
    let mut pruned = MeleeSearcher::new(&valuation, finish());
    let mut unpruned = MeleeSearcher::new(&valuation, finish()).without_opponent_pruning();

    let with = pruned.search_fixed(&board, 2);
    let without = unpruned.search_fixed(&board, 2);

    assert!(
        with.nodes_explored < without.nodes_explored,
        "pruned {} vs unpruned {}",
        with.nodes_explored,
        without.nodes_explored
    );
    assert_eq!(with.best, without.best);
    assert_matches_reference(&board, 2);
}

#[test]
fn an_opponent_with_no_safe_heading_keeps_all_four_headings() {
    // The corner rival is boxed in by walls, its own body and a stacked tail
    // that does not vacate this turn: it dies whatever it does.
    let us: &[(i32, i32)] = &[(4, 0), (3, 0), (2, 0)];
    let boxed: &[(i32, i32)] = &[(10, 0), (10, 1), (9, 1), (9, 0), (9, 0)];
    let far: &[(i32, i32)] = &[(9, 9), (9, 8), (9, 7)];
    let state = turn_state_from_bodies(&[us, boxed, far], &[90, 90, 90], 0, &[]);
    let board = ingest_melee(&state).expect("melee");

    assert!(
        !Heading::ALL
            .into_iter()
            .any(|h| is_self_preserving(&board, Seat::ALL[1], h))
    );
    assert_matches_reference(&board, 2);
}

/// Whether `ours` keeps us alive against every joint reply.
fn survives_every_reply(board: &MeleeBoard, ours: Heading) -> bool {
    joint_replies(board, ours)
        .into_iter()
        .all(|moves| !matches!(board.advance(&moves), MeleeOutcome::WeDown { .. }))
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(40))]

    #[test]
    fn depth_one_agrees_with_the_reference_and_never_loses_when_a_heading_survives(
        spec in melee_spec(),
    ) {
        let board = ingest_melee(&realize_melee(&spec)).expect("melee");
        assert_matches_reference(&board, 1);
        let valuation = MeleeValuation::standard();
        let mut searcher = MeleeSearcher::new(&valuation, finish());
        let report = searcher.search_fixed(&board, 1);
        if Heading::ALL.into_iter().any(|h| survives_every_reply(&board, h)) {
            let best = report.best.expect("completed");
            prop_assert!(survives_every_reply(&board, best), "{best:?} does not survive");
            prop_assert!(report.principal_score.expect("completed") > -finish().finite_limit());
        }
    }

    #[test]
    fn depth_two_agrees_with_the_reference_on_three_snake_melees(spec in melee_spec()) {
        let mut spec = spec;
        spec.snakes.truncate(3);
        spec.you %= 3;
        let board = ingest_melee(&realize_melee(&spec)).expect("melee");
        assert_matches_reference(&board, 2);
    }
}
