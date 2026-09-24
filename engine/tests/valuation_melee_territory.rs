mod support;

use std::collections::HashMap;

use proptest::prelude::*;
use tiger_engine::arena::cellset::{Cell, CellSet};
use tiger_engine::arena::ingest::ingest_melee;
use tiger_engine::arena::melee::{MAX_SEATS, MeleeBoard, Seat};
use tiger_engine::rules_core::TurnState;
use tiger_engine::valuation::Assessor;
use tiger_engine::valuation::melee::Surveyed;
use tiger_engine::valuation::melee::territory::{MeleeSurvey, Territory};

use support::{melee_spec, realize_melee, seat_ids, turn_state_from_bodies};

fn cell(x: u8, y: u8) -> Cell {
    Cell::from_xy(x, y).expect("on the board")
}

fn melee(bodies: &[&[(i32, i32)]], food: &[(i32, i32)]) -> (TurnState, MeleeBoard) {
    let health = vec![90; bodies.len()];
    let state = turn_state_from_bodies(bodies, &health, 0, food);
    let board = ingest_melee(&state).expect("a melee converts");
    (state, board)
}

#[test]
fn three_serpents_in_three_corners_split_the_board_by_arrival_and_length() {
    // Us in the middle of the bottom edge, a longer serpent top-left, a shorter one top-right.
    let us: &[(i32, i32)] = &[(5, 0), (5, 1), (5, 2)];
    let long: &[(i32, i32)] = &[(0, 10), (0, 9), (0, 8), (0, 7)];
    let short: &[(i32, i32)] = &[(10, 10), (10, 9), (10, 8)];
    let (_, board) = melee(&[us, long, short], &[(5, 3), (0, 5)]);

    let survey = Territory.survey(&board);

    // Behind our tail (free from turn 3, reached on turn 4 by waiting up the
    // body), far from both: ours. Beside the long serpent's head: its.
    assert!(survey.owned[0].contains(cell(5, 3)));
    assert!(survey.owned[1].contains(cell(1, 10)));
    assert!(survey.owned[2].contains(cell(9, 10)));
    // (5,10) is 10 steps from us and 5 from each corner serpent: the longer one takes it.
    assert!(survey.owned[1].contains(cell(5, 10)));
    assert!(!survey.owned[2].contains(cell(5, 10)));
    // (5,5): 5 from us, 10 from the corners: ours.
    assert!(survey.owned[0].contains(cell(5, 5)));
    assert_eq!(survey.food[0], Some(4));
    assert_eq!(survey.owned[3], CellSet::EMPTY);
    assert_eq!(survey.food[3], None);
    // We hold 57 cells to their 32 and 28: more than either of them, and just
    // short of the two together, which is what the term reads (iteration 18).
    let ours = survey.owned[0].len().cast_signed();
    let held_against_us: i32 = (1..3)
        .map(|seat| survey.owned[seat].len().cast_signed())
        .sum();
    assert!(
        ours > survey.owned[1].len().cast_signed(),
        "we own more than either rival on its own"
    );
    assert_eq!(
        Territory.assess(&Surveyed::new(&board)),
        ours - held_against_us
    );
}

#[test]
fn equal_length_seats_arriving_together_own_nothing_there() {
    let west: &[(i32, i32)] = &[(3, 5), (2, 5), (1, 5)];
    let east: &[(i32, i32)] = &[(7, 5), (8, 5), (9, 5)];
    let far: &[(i32, i32)] = &[(5, 10), (5, 9), (5, 8)];
    let (_, board) = melee(&[west, east, far], &[]);

    let survey = Territory.survey(&board);

    assert!(!survey.owned[0].contains(cell(5, 5)));
    assert!(!survey.owned[1].contains(cell(5, 5)));
    assert!(survey.owned[0].contains(cell(4, 5)));
    assert!(survey.owned[1].contains(cell(6, 5)));
}

// --- Independent oracle: earliest arrival per seat by relaxation over the freeing schedule. ---

/// The turn each body cell frees: one past the largest tail index of a copy of it.
fn free_times(state: &TurnState) -> HashMap<Cell, u16> {
    let mut times = HashMap::new();
    for snake in state.snakes() {
        let body = snake.body();
        let length = body.len();
        for (index, segment) in body.iter().enumerate() {
            let from_tail = (length - 1 - index) as u16;
            let cell = Cell::from_index(segment.value()).expect("on the board");
            let entry = times.entry(cell).or_insert(0);
            *entry = (*entry).max(from_tail + 1);
        }
    }
    times
}

/// Earliest turn `head`'s serpent can stand on each cell: a cell is entered at
/// `max(free time, 1 + earliest neighbour)`; waiting beside it is allowed.
fn arrivals(head: Cell, free: &HashMap<Cell, u16>) -> HashMap<Cell, u16> {
    let mut best: HashMap<Cell, u16> = HashMap::new();
    best.insert(head, 0);
    let mut changed = true;
    while changed {
        changed = false;
        for cell in CellSet::BOARD {
            if cell == head {
                continue;
            }
            let via = CellSet::single(cell)
                .neighbours()
                .into_iter()
                .filter_map(|n| best.get(&n).map(|t| t + 1))
                .min();
            let Some(via) = via else { continue };
            let candidate = via.max(free.get(&cell).copied().unwrap_or(0));
            if best.get(&cell).is_none_or(|t| candidate < *t) {
                best.insert(cell, candidate);
                changed = true;
            }
        }
    }
    best
}

fn oracle(state: &TurnState) -> MeleeSurvey {
    let board = ingest_melee(state).expect("melee");
    let free = free_times(state);
    let ids = seat_ids(state);
    let seats: Vec<Seat> = board.seats().collect();
    let by_seat: Vec<(HashMap<Cell, u16>, u8, Cell)> = seats
        .iter()
        .map(|seat| {
            let serpent = board.serpent(*seat);
            (
                arrivals(serpent.head(), &free),
                serpent.length(),
                serpent.head(),
            )
        })
        .collect();
    let heads: Vec<Cell> = by_seat.iter().map(|(_, _, h)| *h).collect();
    let mut survey = MeleeSurvey {
        owned: [CellSet::EMPTY; MAX_SEATS],
        food: [None; MAX_SEATS],
    };
    let food = CellSet::from_bits(state.food().bits());
    for cell in CellSet::BOARD {
        if heads.contains(&cell) {
            continue;
        }
        let reached: Vec<(usize, u16, u8)> = by_seat
            .iter()
            .enumerate()
            .filter_map(|(i, (arrival, length, _))| arrival.get(&cell).map(|t| (i, *t, *length)))
            .collect();
        let Some(earliest) = reached.iter().map(|(_, t, _)| *t).min() else {
            continue;
        };
        let first: Vec<&(usize, u16, u8)> =
            reached.iter().filter(|(_, t, _)| *t == earliest).collect();
        let longest = first.iter().map(|(_, _, l)| *l).max().expect("non-empty");
        let winners: Vec<usize> = first
            .iter()
            .filter(|(_, _, l)| *l == longest)
            .map(|(i, _, _)| *i)
            .collect();
        if winners.len() == 1 {
            let seat = seats[winners[0]].index();
            survey.owned[seat] = survey.owned[seat].with(cell);
            if food.contains(cell) {
                survey.food[seat] = Some(survey.food[seat].map_or(earliest, |d| d.min(earliest)));
            }
        }
    }
    let _ = ids;
    survey
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(150))]

    #[test]
    fn the_survey_agrees_with_the_arrival_oracle_on_generated_melees(spec in melee_spec()) {
        let state = realize_melee(&spec);
        let board = ingest_melee(&state).expect("melee");
        prop_assert_eq!(Territory.survey(&board), oracle(&state));
    }
}

#[test]
fn territory_answers_to_every_rival_at_once_not_only_to_the_largest() {
    // Growth iteration 18. The four corners, us the longest so every tie falls
    // our way: we own 34 cells and the three of them own 24, 25 and 25. Read
    // against the largest rival alone the position is comfortable, +9; read
    // against the board it is what being hemmed in looks like, and the cage
    // that kills us is built by two rivals at once, never by the biggest one
    // on its own.
    let us: &[(i32, i32)] = &[(0, 0), (0, 1), (0, 2), (0, 3), (0, 4)];
    let east: &[(i32, i32)] = &[(10, 0), (10, 1), (10, 2)];
    let north: &[(i32, i32)] = &[(0, 10), (1, 10), (2, 10)];
    let far: &[(i32, i32)] = &[(10, 10), (9, 10), (8, 10)];
    let (_, board) = melee(&[us, east, north, far], &[]);

    let survey = Territory.survey(&board);
    let ours = survey.owned[0].len();
    let rivals: Vec<u32> = (1..4).map(|seat| survey.owned[seat].len()).collect();

    assert!(
        rivals.iter().all(|rival| *rival < ours),
        "the position is built so that no single rival owns as much as we do"
    );
    assert!(
        ours < rivals.iter().sum::<u32>(),
        "and so that the three of them together own more"
    );
    assert_eq!(
        Territory.assess(&Surveyed::new(&board)),
        ours.cast_signed() - rivals.iter().sum::<u32>().cast_signed(),
        "the term is our ground against all the ground held against us"
    );
}
