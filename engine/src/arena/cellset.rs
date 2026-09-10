//! Cells of the 11x11 board and sets of cells packed into one `u128`.
//!
//! A cell index is `y * 11 + x`, so bits 0..=120 are valid and bits 121..=127
//! must stay zero after every operation.

pub const BOARD_SIDE: u8 = 11;
pub const CELL_COUNT: u8 = BOARD_SIDE * BOARD_SIDE;

const VALID_BITS: u128 = (1u128 << CELL_COUNT) - 1;

const fn column_mask(x: u32) -> u128 {
    let mut mask = 0u128;
    let mut y = 0u32;
    while y < BOARD_SIDE as u32 {
        mask |= 1u128 << (y * BOARD_SIDE as u32 + x);
        y += 1;
    }
    mask
}

const WEST_COLUMN: u128 = column_mask(0);
const EAST_COLUMN: u128 = column_mask(BOARD_SIDE as u32 - 1);

/// One board cell; the index is always in `0..=120`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Cell(u8);

impl Cell {
    #[must_use]
    pub const fn from_index(index: u8) -> Option<Self> {
        if index < CELL_COUNT {
            Some(Self(index))
        } else {
            None
        }
    }

    #[must_use]
    pub const fn from_xy(x: u8, y: u8) -> Option<Self> {
        if x < BOARD_SIDE && y < BOARD_SIDE {
            Some(Self(y * BOARD_SIDE + x))
        } else {
            None
        }
    }

    #[must_use]
    pub const fn index(self) -> u8 {
        self.0
    }

    #[must_use]
    pub const fn x(self) -> u8 {
        self.0 % BOARD_SIDE
    }

    #[must_use]
    pub const fn y(self) -> u8 {
        self.0 / BOARD_SIDE
    }
}

/// A set of board cells.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct CellSet(u128);

impl CellSet {
    pub const EMPTY: Self = Self(0);
    pub const BOARD: Self = Self(VALID_BITS);

    /// Keeps only the bits that name real cells.
    #[must_use]
    pub const fn from_bits(bits: u128) -> Self {
        Self(bits & VALID_BITS)
    }

    #[must_use]
    pub const fn bits(self) -> u128 {
        self.0
    }

    #[must_use]
    pub const fn single(cell: Cell) -> Self {
        Self(1u128 << cell.0)
    }

    #[must_use]
    pub const fn contains(self, cell: Cell) -> bool {
        self.0 >> cell.0 & 1 == 1
    }

    #[must_use]
    pub const fn with(self, cell: Cell) -> Self {
        Self(self.0 | 1u128 << cell.0)
    }

    #[must_use]
    pub const fn without(self, cell: Cell) -> Self {
        Self(self.0 & !(1u128 << cell.0))
    }

    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    #[must_use]
    pub const fn intersection(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }

    #[must_use]
    pub const fn difference(self, other: Self) -> Self {
        Self(self.0 & !other.0)
    }

    #[must_use]
    pub const fn complement(self) -> Self {
        Self(!self.0 & VALID_BITS)
    }

    #[must_use]
    pub const fn len(self) -> u32 {
        self.0.count_ones()
    }

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    #[must_use]
    pub const fn north(self) -> Self {
        Self(self.0 << BOARD_SIDE & VALID_BITS)
    }

    #[must_use]
    pub const fn south(self) -> Self {
        Self(self.0 >> BOARD_SIDE)
    }

    #[must_use]
    pub const fn east(self) -> Self {
        Self((self.0 & !EAST_COLUMN) << 1)
    }

    #[must_use]
    pub const fn west(self) -> Self {
        Self((self.0 & !WEST_COLUMN) >> 1)
    }

    /// Every cell orthogonally adjacent to a member (members themselves are
    /// included only when adjacent to another member).
    #[must_use]
    pub const fn neighbours(self) -> Self {
        self.north()
            .union(self.east())
            .union(self.south())
            .union(self.west())
    }

    /// The set plus all of its neighbours: one flood-fill step.
    #[must_use]
    pub const fn expanded(self) -> Self {
        self.union(self.neighbours())
    }

    #[must_use]
    pub const fn iter(self) -> Cells {
        Cells(self.0)
    }
}

/// Ascending iteration over the members of a `CellSet`.
#[derive(Clone, Copy, Debug)]
pub struct Cells(u128);

impl Iterator for Cells {
    type Item = Cell;

    fn next(&mut self) -> Option<Cell> {
        if self.0 == 0 {
            return None;
        }
        let index = self.0.trailing_zeros() as u8;
        self.0 &= self.0 - 1;
        Some(Cell(index))
    }
}

impl IntoIterator for CellSet {
    type Item = Cell;
    type IntoIter = Cells;

    fn into_iter(self) -> Cells {
        self.iter()
    }
}
