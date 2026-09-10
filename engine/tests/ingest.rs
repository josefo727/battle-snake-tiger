mod support;

use tiger_engine::arena::cellset::{Cell, CellSet};
use tiger_engine::arena::duel::{BoardError, DuelBoard, Side};
use tiger_engine::arena::ingest::{IngestError, ingest};
use tiger_engine::arena::serpent::Serpent;

use support::turn_state;

fn cell(x: u8, y: u8) -> Cell {
    Cell::from_xy(x, y).expect("test coordinate is on the board")
}

fn cells(coordinates: &[(u8, u8)]) -> Vec<Cell> {
    coordinates.iter().map(|&(x, y)| cell(x, y)).collect()
}

fn set_of(coordinates: &[(u8, u8)]) -> CellSet {
    cells(coordinates)
        .into_iter()
        .fold(CellSet::EMPTY, CellSet::with)
}

fn serpent(coordinates: &[(u8, u8)], vigor: u8) -> Serpent {
    Serpent::new(&cells(coordinates), vigor).expect("test serpent is valid")
}

const US: [(u8, u8); 3] = [(5, 5), (5, 4), (5, 3)];
const THEM: [(u8, u8); 3] = [(2, 2), (2, 1), (2, 0)];

#[test]
fn a_valid_board_starts_at_ply_zero_and_reports_combined_occupancy() {
    let board = DuelBoard::try_new(serpent(&US, 90), serpent(&THEM, 80), set_of(&[(10, 10)]))
        .expect("the board is valid");

    assert_eq!(board.ply(), 0);
    assert_eq!(board.pellets(), set_of(&[(10, 10)]));
    assert_eq!(board.occupied(), set_of(&US).union(set_of(&THEM)));
    assert_eq!(board.serpent(Side::Us).vigor(), 90);
    assert_eq!(board.serpent(Side::Them).vigor(), 80);
}

#[test]
fn construction_rejects_serpents_that_share_a_cell() {
    let overlapping: [(u8, u8); 3] = [(5, 3), (4, 3), (3, 3)];

    let result = DuelBoard::try_new(serpent(&US, 90), serpent(&overlapping, 90), CellSet::EMPTY);

    assert_eq!(result.unwrap_err(), BoardError::SerpentsOverlap);
}

#[test]
fn construction_rejects_a_pellet_under_a_serpent() {
    let result = DuelBoard::try_new(serpent(&US, 90), serpent(&THEM, 90), set_of(&[(5, 4)]));

    assert_eq!(result.unwrap_err(), BoardError::PelletUnderSerpent);
}

#[test]
fn construction_rejects_vigor_outside_one_to_one_hundred() {
    for vigor in [0, 101] {
        let result = DuelBoard::try_new(serpent(&US, vigor), serpent(&THEM, 90), CellSet::EMPTY);
        assert_eq!(
            result.unwrap_err(),
            BoardError::VigorOutOfRange,
            "vigor {vigor}"
        );
    }
    for vigor in [1, 100] {
        assert!(
            DuelBoard::try_new(serpent(&US, vigor), serpent(&THEM, 90), CellSet::EMPTY).is_ok()
        );
    }
}

#[test]
fn ingest_preserves_ordered_bodies_health_and_food_with_our_snake_first() {
    let state = turn_state(2, 0, &[(10, 10), (0, 0)]);

    let board = ingest(&state).expect("a duel converts");

    let us: Vec<Cell> = board.serpent(Side::Us).body().collect();
    let them: Vec<Cell> = board.serpent(Side::Them).body().collect();
    assert_eq!(us, cells(&US));
    assert_eq!(them, cells(&THEM));
    assert_eq!(board.serpent(Side::Us).vigor(), 90);
    assert_eq!(board.pellets(), set_of(&[(10, 10), (0, 0)]));
    assert_eq!(board.ply(), 0);
}

#[test]
fn ingest_puts_our_snake_first_even_when_it_is_listed_second() {
    let state = turn_state(2, 1, &[]);

    let board = ingest(&state).expect("a duel converts");

    let us: Vec<Cell> = board.serpent(Side::Us).body().collect();
    assert_eq!(us, cells(&THEM));
}

#[test]
fn ingested_occupancy_equals_the_reused_states_occupancy() {
    let state = turn_state(2, 0, &[]);

    let board = ingest(&state).expect("a duel converts");

    assert_eq!(
        board.occupied(),
        CellSet::from_bits(state.occupancy().bits())
    );
}

#[test]
fn ingest_rejects_three_and_four_snake_states() {
    for snakes in [3, 4] {
        let state = turn_state(snakes, 0, &[]);
        assert_eq!(
            ingest(&state).unwrap_err(),
            IngestError::NotADuel,
            "{snakes} snakes"
        );
    }
}
