use tiger_engine::arena::cellset::{Cell, CellSet};
use tiger_engine::arena::duel::{BoardError, Side};
use tiger_engine::arena::melee::{MAX_SEATS, MeleeBoard, Seat};
use tiger_engine::arena::serpent::Serpent;

fn cell(x: u8, y: u8) -> Cell {
    Cell::from_xy(x, y).expect("test coordinate is on the board")
}

fn set_of(coordinates: &[(u8, u8)]) -> CellSet {
    coordinates
        .iter()
        .map(|&(x, y)| cell(x, y))
        .fold(CellSet::EMPTY, CellSet::with)
}

fn serpent_with(coordinates: &[(u8, u8)], vigor: u8) -> Serpent {
    let body: Vec<Cell> = coordinates.iter().map(|&(x, y)| cell(x, y)).collect();
    Serpent::new(&body, vigor).expect("test serpent is valid")
}

fn serpent(coordinates: &[(u8, u8)]) -> Serpent {
    serpent_with(coordinates, 90)
}

const US: &[(u8, u8)] = &[(5, 5), (5, 4), (5, 3)];
const EAST: &[(u8, u8)] = &[(9, 9), (9, 8), (9, 7)];
const WEST: &[(u8, u8)] = &[(1, 9), (1, 8), (1, 7)];
const SOUTH: &[(u8, u8)] = &[(2, 2), (2, 1), (2, 0)];

fn three() -> MeleeBoard {
    MeleeBoard::try_new(
        &[serpent(US), serpent(EAST), serpent(WEST)],
        set_of(&[(0, 0)]),
    )
    .expect("three serpents apart are a valid melee")
}

#[test]
fn seats_are_the_four_positions_with_ours_first() {
    assert_eq!(Seat::ALL.len(), MAX_SEATS);
    assert_eq!(Seat::US, Seat::ALL[0]);
    assert_eq!(Seat::new(3).map(Seat::index), Some(3));
    assert_eq!(Seat::new(4), None);
}

#[test]
fn a_melee_needs_two_to_four_serpents() {
    let one = MeleeBoard::try_new(&[serpent(US)], CellSet::EMPTY);
    let five = MeleeBoard::try_new(
        &[
            serpent(US),
            serpent(EAST),
            serpent(WEST),
            serpent(SOUTH),
            serpent(&[(7, 2), (7, 1), (7, 0)]),
        ],
        CellSet::EMPTY,
    );
    let two = MeleeBoard::try_new(&[serpent(US), serpent(EAST)], CellSet::EMPTY);
    let four = MeleeBoard::try_new(
        &[serpent(US), serpent(EAST), serpent(WEST), serpent(SOUTH)],
        CellSet::EMPTY,
    );

    assert_eq!(one.unwrap_err(), BoardError::SeatCount);
    assert_eq!(five.unwrap_err(), BoardError::SeatCount);
    assert_eq!(two.expect("two is a melee too").alive_count(), 2);
    assert_eq!(four.expect("four is the most").alive_count(), 4);
}

#[test]
fn the_duel_invariants_hold_for_every_seat() {
    let overlap = MeleeBoard::try_new(
        &[
            serpent(US),
            serpent(EAST),
            serpent(&[(9, 7), (9, 6), (9, 5)]),
        ],
        CellSet::EMPTY,
    );
    let pellet_under_third = MeleeBoard::try_new(
        &[serpent(US), serpent(EAST), serpent(WEST)],
        set_of(&[(1, 8)]),
    );
    let starved_third = MeleeBoard::try_new(
        &[serpent(US), serpent(EAST), serpent_with(WEST, 0)],
        CellSet::EMPTY,
    );

    assert_eq!(overlap.unwrap_err(), BoardError::SerpentsOverlap);
    assert_eq!(
        pellet_under_third.unwrap_err(),
        BoardError::PelletUnderSerpent
    );
    assert_eq!(starved_third.unwrap_err(), BoardError::VigorOutOfRange);
}

#[test]
fn every_living_body_cell_is_occupied() {
    let board = three();

    let expected = set_of(US).union(set_of(EAST)).union(set_of(WEST));
    assert_eq!(board.occupied(), expected);
    assert_eq!(board.pellets(), set_of(&[(0, 0)]));
    assert_eq!(board.ply(), 0);
}

#[test]
fn the_living_seats_come_in_seat_order_and_carry_their_serpents() {
    let board = three();

    let seats: Vec<Seat> = board.seats().collect();
    assert_eq!(seats, vec![Seat::ALL[0], Seat::ALL[1], Seat::ALL[2]]);
    assert!(board.is_alive(Seat::ALL[2]));
    assert!(!board.is_alive(Seat::ALL[3]));
    assert_eq!(board.serpent(Seat::ALL[1]).head(), cell(9, 9));
}

#[test]
fn a_two_seat_position_converts_to_a_duel_with_our_seat_first() {
    let board =
        MeleeBoard::try_new(&[serpent(US), serpent(WEST)], set_of(&[(0, 0)])).expect("valid");

    let duel = board.as_duel();

    assert_eq!(duel.serpent(Side::Us).head(), cell(5, 5));
    assert_eq!(duel.serpent(Side::Them).head(), cell(1, 9));
    assert_eq!(duel.pellets(), set_of(&[(0, 0)]));
    assert_eq!(duel.ply(), board.ply());
}
