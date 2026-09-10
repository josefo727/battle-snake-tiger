use std::collections::BTreeSet;

use proptest::prelude::*;
use tiger_engine::arena::cellset::{CELL_COUNT, Cell, CellSet};

fn cell(x: u8, y: u8) -> Cell {
    Cell::from_xy(x, y).expect("test coordinate is on the board")
}

fn set_of(cells: &[(u8, u8)]) -> CellSet {
    cells
        .iter()
        .fold(CellSet::EMPTY, |set, &(x, y)| set.with(cell(x, y)))
}

/// An oracle that reads raw bits directly and shares no code with `CellSet`.
fn members(bits: u128) -> BTreeSet<u8> {
    (0..CELL_COUNT).filter(|i| bits >> i & 1 == 1).collect()
}

fn any_set() -> impl Strategy<Value = CellSet> {
    any::<u128>().prop_map(CellSet::from_bits)
}

fn any_cell() -> impl Strategy<Value = Cell> {
    (0..CELL_COUNT).prop_map(|i| Cell::from_index(i).expect("index is in range"))
}

#[test]
fn cell_coordinates_round_trip_for_every_index() {
    for index in 0..CELL_COUNT {
        let cell = Cell::from_index(index).expect("index is on the board");
        assert_eq!(Cell::from_xy(cell.x(), cell.y()), Some(cell));
        assert_eq!(cell.index(), cell.y() * 11 + cell.x());
    }
}

#[test]
fn cells_outside_the_board_are_rejected() {
    assert_eq!(Cell::from_index(CELL_COUNT), None);
    assert_eq!(Cell::from_xy(11, 0), None);
    assert_eq!(Cell::from_xy(0, 11), None);
}

#[test]
fn corner_cells_have_two_neighbours() {
    for (x, y) in [(0, 0), (10, 0), (0, 10), (10, 10)] {
        let neighbours = CellSet::single(cell(x, y)).neighbours();
        assert_eq!(neighbours.len(), 2, "corner ({x}, {y})");
    }
}

#[test]
fn corner_neighbours_are_the_two_adjacent_cells() {
    let neighbours = CellSet::single(cell(0, 0)).neighbours();

    assert_eq!(neighbours, set_of(&[(1, 0), (0, 1)]));
}

#[test]
fn edge_cells_have_three_neighbours_and_interior_cells_four() {
    assert_eq!(CellSet::single(cell(0, 5)).neighbours().len(), 3);
    assert_eq!(CellSet::single(cell(5, 10)).neighbours().len(), 3);
    assert_eq!(CellSet::single(cell(5, 5)).neighbours().len(), 4);
}

#[test]
fn horizontal_neighbours_do_not_wrap_across_rows() {
    let east_edge = CellSet::single(cell(10, 3));
    let west_edge = CellSet::single(cell(0, 4));

    assert_eq!(east_edge.east(), CellSet::EMPTY);
    assert_eq!(west_edge.west(), CellSet::EMPTY);
    assert!(!east_edge.neighbours().contains(cell(0, 4)));
    assert!(!west_edge.neighbours().contains(cell(10, 3)));
}

#[test]
fn vertical_shifts_drop_cells_that_leave_the_board() {
    assert_eq!(CellSet::single(cell(4, 10)).north(), CellSet::EMPTY);
    assert_eq!(CellSet::single(cell(4, 0)).south(), CellSet::EMPTY);
    assert_eq!(
        CellSet::single(cell(4, 4)).north(),
        CellSet::single(cell(4, 5))
    );
}

#[test]
fn iteration_yields_cells_in_ascending_order_and_len_counts_them() {
    let set = set_of(&[(10, 10), (0, 0), (5, 5), (3, 7)]);

    let indices: Vec<u8> = set.iter().map(Cell::index).collect();

    assert_eq!(indices, vec![0, 60, 80, 120]);
    assert_eq!(set.len(), 4);
}

#[test]
fn expanded_keeps_the_original_members() {
    let set = set_of(&[(2, 2), (8, 8)]);

    assert_eq!(set.expanded().intersection(set), set);
    assert_eq!(set.expanded().len(), 10);
}

proptest! {
    #[test]
    fn no_operation_ever_sets_a_bit_above_the_board(a in any_set(), b in any_set()) {
        let results = [
            a.union(b), a.intersection(b), a.difference(b), a.complement(),
            a.north(), a.south(), a.east(), a.west(), a.neighbours(), a.expanded(),
        ];
        for result in results {
            prop_assert_eq!(result.bits() >> CELL_COUNT, 0);
        }
    }

    #[test]
    fn set_algebra_matches_a_btreeset_model(a in any_set(), b in any_set()) {
        let (ma, mb) = (members(a.bits()), members(b.bits()));

        prop_assert_eq!(members(a.union(b).bits()), &ma | &mb);
        prop_assert_eq!(members(a.intersection(b).bits()), &ma & &mb);
        prop_assert_eq!(members(a.difference(b).bits()), &ma - &mb);
        prop_assert_eq!(a.len() as usize, ma.len());
    }

    #[test]
    fn a_set_and_its_complement_partition_the_board(a in any_set()) {
        prop_assert_eq!(a.union(a.complement()), CellSet::BOARD);
        prop_assert_eq!(a.intersection(a.complement()), CellSet::EMPTY);
        prop_assert_eq!(a.complement().complement(), a);
    }

    #[test]
    fn iteration_agrees_with_the_bit_oracle(a in any_set()) {
        let iterated: BTreeSet<u8> = a.iter().map(Cell::index).collect();

        prop_assert_eq!(iterated, members(a.bits()));
    }

    #[test]
    fn the_neighbour_relation_is_symmetric(a in any_cell(), b in any_cell()) {
        let a_sees_b = CellSet::single(a).neighbours().contains(b);
        let b_sees_a = CellSet::single(b).neighbours().contains(a);

        prop_assert_eq!(a_sees_b, b_sees_a);
    }
}
