use std::collections::VecDeque;

use proptest::prelude::*;
use tiger_engine::arena::cellset::{Cell, CellSet};
use tiger_engine::arena::heading::Heading;
use tiger_engine::arena::serpent::{MAX_LENGTH, Serpent, SerpentError};

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

fn serpent(coordinates: &[(u8, u8)]) -> Serpent {
    Serpent::new(&cells(coordinates), 100).expect("test serpent is valid")
}

fn body_of(serpent: &Serpent) -> Vec<Cell> {
    serpent.body().collect()
}

/// One legal move: new head, release the old tail.
fn slide(serpent: &mut Serpent, to: Cell) -> Cell {
    serpent.advance_head(to);
    serpent.release_tail()
}

#[test]
fn a_new_serpent_preserves_body_order_and_mirrors_occupancy() {
    let s = serpent(&[(5, 5), (5, 4), (5, 3)]);

    assert_eq!(body_of(&s), cells(&[(5, 5), (5, 4), (5, 3)]));
    assert_eq!(s.head(), cell(5, 5));
    assert_eq!(s.tail(), cell(5, 3));
    assert_eq!(s.length(), 3);
    assert_eq!(s.vigor(), 100);
    assert_eq!(s.cells(), set_of(&[(5, 5), (5, 4), (5, 3)]));
}

#[test]
fn construction_rejects_malformed_bodies() {
    let too_long: Vec<Cell> = (0..=MAX_LENGTH)
        .map(|i| Cell::from_index(i % 121).unwrap())
        .collect();

    assert_eq!(Serpent::new(&[], 100).unwrap_err(), SerpentError::EmptyBody);
    assert_eq!(
        Serpent::new(&too_long, 100).unwrap_err(),
        SerpentError::TooLong
    );
    assert_eq!(
        Serpent::new(&cells(&[(5, 5), (5, 3)]), 100).unwrap_err(),
        SerpentError::DisconnectedBody
    );
    assert_eq!(
        Serpent::new(&cells(&[(5, 5), (5, 4), (6, 4), (6, 5), (5, 5)]), 100).unwrap_err(),
        SerpentError::SelfOverlap
    );
}

#[test]
fn an_ordinary_move_shifts_the_body_and_frees_the_tail_cell() {
    let mut s = serpent(&[(5, 5), (5, 4), (5, 3)]);

    let released = slide(&mut s, cell(5, 6));

    assert_eq!(released, cell(5, 3));
    assert_eq!(body_of(&s), cells(&[(5, 6), (5, 5), (5, 4)]));
    assert_eq!(s.length(), 3);
    assert_eq!(s.cells(), set_of(&[(5, 6), (5, 5), (5, 4)]));
    assert!(!s.cells().contains(cell(5, 3)));
}

#[test]
fn a_head_moving_onto_the_vacating_tail_keeps_that_cell_occupied() {
    let mut s = serpent(&[(1, 1), (1, 0), (0, 0), (0, 1)]);

    slide(&mut s, cell(0, 1));

    assert_eq!(body_of(&s), cells(&[(0, 1), (1, 1), (1, 0), (0, 0)]));
    assert!(s.cells().contains(cell(0, 1)));
    assert_eq!(s.cells(), set_of(&[(0, 1), (1, 1), (1, 0), (0, 0)]));
}

#[test]
fn a_stacked_tail_keeps_its_cell_until_the_second_copy_is_released() {
    let mut s = serpent(&[(5, 5), (5, 4), (5, 3)]);

    s.stack_tail().expect("room to grow");
    assert_eq!(body_of(&s), cells(&[(5, 5), (5, 4), (5, 3), (5, 3)]));
    assert_eq!(s.length(), 4);
    assert_eq!(s.cells(), set_of(&[(5, 5), (5, 4), (5, 3)]));

    slide(&mut s, cell(5, 6));
    assert_eq!(body_of(&s), cells(&[(5, 6), (5, 5), (5, 4), (5, 3)]));
    assert!(s.cells().contains(cell(5, 3)));

    slide(&mut s, cell(5, 7));
    assert!(!s.cells().contains(cell(5, 3)));
}

#[test]
fn a_coiled_start_releases_one_copy_per_move() {
    let mut s = serpent(&[(1, 5), (1, 5), (1, 5)]);
    assert_eq!(s.cells(), set_of(&[(1, 5)]));

    slide(&mut s, cell(2, 5));

    assert_eq!(body_of(&s), cells(&[(2, 5), (1, 5), (1, 5)]));
    assert_eq!(s.cells(), set_of(&[(2, 5), (1, 5)]));
}

#[test]
fn a_serpent_filling_the_board_cannot_grow_further() {
    let mut order: Vec<Cell> = Vec::new();
    for y in 0..11u8 {
        let row: Vec<Cell> = (0..11u8).map(|x| cell(x, y)).collect();
        if y % 2 == 0 {
            order.extend(row);
        } else {
            order.extend(row.into_iter().rev());
        }
    }
    order.reverse(); // head first
    let mut s = Serpent::new(&order, 100).expect("a full-board serpent is valid");

    assert_eq!(s.length(), MAX_LENGTH);
    assert_eq!(s.stack_tail(), Err(SerpentError::TooLong));
}

fn any_heading() -> impl Strategy<Value = Heading> {
    (0usize..4).prop_map(|i| Heading::ALL[i])
}

proptest! {
    #[test]
    fn ring_length_and_occupancy_track_a_deque_model_through_legal_play(
        start_x in 3u8..8,
        start_y in 6u8..=10,
        length in 1usize..=6,
        moves in proptest::collection::vec((any_heading(), any::<bool>()), 0..80),
    ) {
        let mut model: VecDeque<Cell> = (0..length)
            .map(|i| cell(start_x, start_y - i as u8))
            .collect();
        let mut s = Serpent::new(&model.iter().copied().collect::<Vec<_>>(), 100).unwrap();

        for (heading, grow) in moves {
            let Some(target) = heading.step(*model.front().unwrap()) else { continue };
            let blocked = model.iter().take(model.len() - 1).any(|&c| c == target);
            if blocked || (grow && s.length() == MAX_LENGTH) {
                continue;
            }

            slide(&mut s, target);
            model.push_front(target);
            model.pop_back();
            if grow {
                s.stack_tail().unwrap();
                model.push_back(*model.back().unwrap());
            }

            prop_assert_eq!(body_of(&s), model.iter().copied().collect::<Vec<_>>());
            prop_assert_eq!(usize::from(s.length()), model.len());
            let expected = model.iter().fold(CellSet::EMPTY, |set, &c| set.with(c));
            prop_assert_eq!(s.cells(), expected);
            prop_assert_eq!(s.head(), *model.front().unwrap());
            prop_assert_eq!(s.tail(), *model.back().unwrap());
        }
    }
}

#[test]
fn debug_output_lists_the_body_instead_of_the_whole_ring() {
    let s = serpent(&[(5, 5), (5, 4), (5, 3)]);

    let text = format!("{s:?}");

    assert_eq!(
        text,
        "Serpent { body: [(5, 5), (5, 4), (5, 3)], vigor: 100 }"
    );
}
