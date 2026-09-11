//! A serpent's body as a fixed-size ring buffer plus an occupancy mirror.

use core::fmt;

use super::cellset::{CELL_COUNT, Cell, CellSet};

/// The ring holds up to 128 slots so slot arithmetic is a mask, not a modulo.
const RING_SIZE: usize = 128;
const RING_MASK: usize = RING_SIZE - 1;

/// A serpent can never be longer than the number of cells on the board.
pub const MAX_LENGTH: u8 = CELL_COUNT;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SerpentError {
    EmptyBody,
    TooLong,
    DisconnectedBody,
    SelfOverlap,
}

/// Body cells ordered head first. Consecutive equal cells are a stack (growth
/// or a fresh serpent's coiled start); `cells` is always the set of body cells.
#[derive(Clone, Copy)]
pub struct Serpent {
    ring: [Cell; RING_SIZE],
    head_slot: u8,
    length: u8,
    vigor: u8,
    cells: CellSet,
}

impl Serpent {
    /// Builds a serpent from a head-first body.
    ///
    /// # Errors
    ///
    /// Rejects an empty body, a body longer than the board, consecutive
    /// segments that are neither equal nor adjacent, and a cell that appears
    /// in two separate runs.
    pub fn new(body: &[Cell], vigor: u8) -> Result<Self, SerpentError> {
        if body.is_empty() {
            return Err(SerpentError::EmptyBody);
        }
        if body.len() > usize::from(MAX_LENGTH) {
            return Err(SerpentError::TooLong);
        }

        let mut ring = [body[0]; RING_SIZE];
        let mut cells = CellSet::EMPTY;
        let mut previous: Option<Cell> = None;
        for (offset, &cell) in body.iter().enumerate() {
            if let Some(before) = previous
                && before != cell
            {
                if !CellSet::single(before).neighbours().contains(cell) {
                    return Err(SerpentError::DisconnectedBody);
                }
                if cells.contains(cell) {
                    return Err(SerpentError::SelfOverlap);
                }
            }
            cells = cells.with(cell);
            ring[RING_SIZE - 1 - offset] = cell;
            previous = Some(cell);
        }

        Ok(Self {
            ring,
            head_slot: (RING_SIZE - 1) as u8,
            length: body.len() as u8,
            vigor,
            cells,
        })
    }

    #[must_use]
    pub const fn head(&self) -> Cell {
        self.ring[self.head_slot as usize]
    }

    #[must_use]
    pub const fn tail(&self) -> Cell {
        self.segment(self.length - 1)
    }

    #[must_use]
    pub const fn length(&self) -> u8 {
        self.length
    }

    #[must_use]
    pub const fn vigor(&self) -> u8 {
        self.vigor
    }

    #[must_use]
    pub const fn cells(&self) -> CellSet {
        self.cells
    }

    /// Segments from head to tail.
    #[must_use]
    pub const fn body(&self) -> Body<'_> {
        Body {
            serpent: self,
            next: 0,
        }
    }

    /// Ring position of the slot `behind` places behind the head slot.
    const fn slot_behind_head(&self, behind: usize) -> usize {
        (self.head_slot as usize + RING_SIZE - behind) & RING_MASK
    }

    const fn segment(&self, from_head: u8) -> Cell {
        self.ring[self.slot_behind_head(from_head as usize)]
    }

    /// The segment `index` places from the tail (0 is the tail itself).
    #[must_use]
    pub const fn cell_from_tail(&self, index: u8) -> Cell {
        self.segment(self.length - 1 - index)
    }

    /// Adds a new head; the old tail is still present until `release_tail`.
    pub fn advance_head(&mut self, cell: Cell) {
        self.head_slot = ((usize::from(self.head_slot) + 1) & RING_MASK) as u8;
        self.ring[usize::from(self.head_slot)] = cell;
        self.length += 1;
        self.cells = self.cells.with(cell);
    }

    /// Removes the oldest segment and returns its cell. The cell stays
    /// occupied when another segment (a stacked copy or the new head) shares it.
    pub fn release_tail(&mut self) -> Cell {
        let released = self.tail();
        self.length -= 1;
        // Stacks are contiguous and only a head landing on the vacating tail
        // can share its cell otherwise, so two comparisons decide occupancy.
        if self.tail() != released && self.head() != released {
            self.cells = self.cells.without(released);
        }
        released
    }

    /// Loses one vigor (a turn's hunger), stopping at zero.
    pub fn lose_vigor(&mut self) {
        self.vigor = self.vigor.saturating_sub(1);
    }

    /// Restores vigor and grows by one: the extra segment sits on the current
    /// (post-move) tail.
    pub fn eat(&mut self, full_vigor: u8) {
        self.vigor = full_vigor;
        // A serpent filling the board cannot grow, and no pellet can exist
        // there, so a refusal is unreachable and safely ignored.
        self.stack_tail().ok();
    }

    /// Places an extra segment on top of the current tail (food growth).
    ///
    /// # Errors
    ///
    /// Fails when the serpent already fills the board.
    pub fn stack_tail(&mut self) -> Result<(), SerpentError> {
        if self.length >= MAX_LENGTH {
            return Err(SerpentError::TooLong);
        }
        let slot = self.slot_behind_head(usize::from(self.length));
        self.ring[slot] = self.tail();
        self.length += 1;
        Ok(())
    }
}

impl fmt::Debug for Serpent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let body: Vec<(u8, u8)> = self.body().map(|cell| (cell.x(), cell.y())).collect();
        formatter
            .debug_struct("Serpent")
            .field("body", &body)
            .field("vigor", &self.vigor)
            .finish()
    }
}

/// Head-to-tail iteration over a serpent's segments.
#[derive(Clone, Copy, Debug)]
pub struct Body<'a> {
    serpent: &'a Serpent,
    next: u8,
}

impl Iterator for Body<'_> {
    type Item = Cell;

    fn next(&mut self) -> Option<Cell> {
        if self.next >= self.serpent.length {
            return None;
        }
        let cell = self.serpent.segment(self.next);
        self.next += 1;
        Some(cell)
    }
}
