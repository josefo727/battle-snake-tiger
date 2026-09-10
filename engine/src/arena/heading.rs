//! The four directions a serpent can move, in a fixed order.

use super::cellset::Cell;

/// North is `y + 1`, east is `x + 1`, south is `y - 1`, west is `x - 1`
/// (Battlesnake's up, right, down, left).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Heading {
    North,
    East,
    South,
    West,
}

impl Heading {
    /// Fixed iteration and tie-break order.
    pub const ALL: [Self; 4] = [Self::North, Self::East, Self::South, Self::West];

    /// Stable position in [`Heading::ALL`].
    #[must_use]
    pub const fn index(self) -> usize {
        self as usize
    }

    #[must_use]
    pub const fn opposite(self) -> Self {
        match self {
            Self::North => Self::South,
            Self::East => Self::West,
            Self::South => Self::North,
            Self::West => Self::East,
        }
    }

    /// The neighbouring cell in this heading, or `None` off the board.
    #[must_use]
    pub fn step(self, cell: Cell) -> Option<Cell> {
        let (dx, dy) = self.delta();
        let x = cell.x().checked_add_signed(dx)?;
        let y = cell.y().checked_add_signed(dy)?;
        Cell::from_xy(x, y)
    }

    const fn delta(self) -> (i8, i8) {
        match self {
            Self::North => (0, 1),
            Self::East => (1, 0),
            Self::South => (0, -1),
            Self::West => (-1, 0),
        }
    }
}
