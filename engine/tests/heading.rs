use proptest::prelude::*;
use tiger_engine::arena::cellset::{CELL_COUNT, Cell, CellSet};
use tiger_engine::arena::heading::Heading;

fn cell(x: u8, y: u8) -> Cell {
    Cell::from_xy(x, y).expect("test coordinate is on the board")
}

fn any_cell() -> impl Strategy<Value = Cell> {
    (0..CELL_COUNT).prop_map(|i| Cell::from_index(i).expect("index is in range"))
}

fn any_heading() -> impl Strategy<Value = Heading> {
    (0usize..4).prop_map(|i| Heading::ALL[i])
}

/// The cell set produced by shifting a single cell in a heading.
fn shifted(heading: Heading, origin: Cell) -> CellSet {
    let set = CellSet::single(origin);
    match heading {
        Heading::North => set.north(),
        Heading::East => set.east(),
        Heading::South => set.south(),
        Heading::West => set.west(),
    }
}

#[test]
fn headings_have_a_fixed_order_and_a_matching_index() {
    assert_eq!(
        Heading::ALL,
        [Heading::North, Heading::East, Heading::South, Heading::West]
    );
    for (position, heading) in Heading::ALL.into_iter().enumerate() {
        assert_eq!(heading.index(), position);
    }
}

#[test]
fn an_interior_cell_steps_to_its_four_neighbours() {
    let origin = cell(5, 5);

    assert_eq!(Heading::North.step(origin), Some(cell(5, 6)));
    assert_eq!(Heading::East.step(origin), Some(cell(6, 5)));
    assert_eq!(Heading::South.step(origin), Some(cell(5, 4)));
    assert_eq!(Heading::West.step(origin), Some(cell(4, 5)));
}

#[test]
fn stepping_off_any_edge_returns_none() {
    assert_eq!(Heading::North.step(cell(5, 10)), None);
    assert_eq!(Heading::East.step(cell(10, 5)), None);
    assert_eq!(Heading::South.step(cell(5, 0)), None);
    assert_eq!(Heading::West.step(cell(0, 5)), None);
}

#[test]
fn opposite_pairs_north_south_and_east_west() {
    assert_eq!(Heading::North.opposite(), Heading::South);
    assert_eq!(Heading::South.opposite(), Heading::North);
    assert_eq!(Heading::East.opposite(), Heading::West);
    assert_eq!(Heading::West.opposite(), Heading::East);
}

proptest! {
    #[test]
    fn opposite_is_an_involution_that_never_matches_itself(heading in any_heading()) {
        prop_assert_eq!(heading.opposite().opposite(), heading);
        prop_assert_ne!(heading.opposite(), heading);
    }

    #[test]
    fn stepping_then_stepping_back_returns_the_original_cell(
        origin in any_cell(),
        heading in any_heading(),
    ) {
        if let Some(next) = heading.step(origin) {
            prop_assert_eq!(heading.opposite().step(next), Some(origin));
        }
    }

    #[test]
    fn step_agrees_with_the_cell_set_shifts(origin in any_cell(), heading in any_heading()) {
        let expected = heading.step(origin).map_or(CellSet::EMPTY, CellSet::single);

        prop_assert_eq!(shifted(heading, origin), expected);
    }
}
