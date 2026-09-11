//! Position assessment: a pipeline of single-question assessors combined by an
//! explicit weight sheet. Everything here is integer arithmetic and statically
//! dispatched, so scoring a leaf never allocates or goes through a vtable.

pub mod dominion;
pub mod finish;
pub mod weights;

use crate::arena::duel::DuelBoard;

/// Most terms one ledger can describe.
pub const LEDGER_CAPACITY: usize = 8;

/// One assessor answering one question about a position.
pub trait Assessor {
    /// Name reported in the ledger.
    const NAME: &'static str;

    /// The raw, unweighted answer from our point of view: positive favours us.
    fn assess(&self, board: &DuelBoard) -> i32;
}

/// What one term contributed to a score.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LedgerEntry {
    pub name: &'static str,
    pub raw: i32,
    pub weight: i32,
}

impl LedgerEntry {
    #[must_use]
    pub const fn contribution(self) -> i32 {
        self.raw * self.weight
    }
}

/// Where a pipeline reports each term as it scores.
pub trait LedgerSink {
    fn record(&mut self, entry: LedgerEntry);
}

/// A fixed-capacity, allocation-free record of every term of one score.
#[derive(Clone, Copy, Debug)]
pub struct Ledger {
    entries: [LedgerEntry; LEDGER_CAPACITY],
    len: usize,
}

impl Ledger {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            entries: [LedgerEntry {
                name: "",
                raw: 0,
                weight: 0,
            }; LEDGER_CAPACITY],
            len: 0,
        }
    }

    #[must_use]
    pub fn entries(&self) -> &[LedgerEntry] {
        &self.entries[..self.len]
    }

    /// The sum of every recorded contribution.
    #[must_use]
    pub fn total(&self) -> i32 {
        self.entries()
            .iter()
            .map(|entry| entry.contribution())
            .sum()
    }
}

impl Default for Ledger {
    fn default() -> Self {
        Self::new()
    }
}

impl LedgerSink for Ledger {
    fn record(&mut self, entry: LedgerEntry) {
        debug_assert!(self.len < LEDGER_CAPACITY, "ledger capacity exceeded");
        if self.len < LEDGER_CAPACITY {
            self.entries[self.len] = entry;
            self.len += 1;
        }
    }
}

/// A sink that discards everything: scoring without a ledger costs nothing.
#[derive(Clone, Copy, Debug, Default)]
pub struct Unrecorded;

impl LedgerSink for Unrecorded {
    fn record(&mut self, _entry: LedgerEntry) {}
}

/// An assessor together with its weight.
#[derive(Clone, Copy, Debug)]
pub struct Weighted<A> {
    pub assessor: A,
    pub weight: i32,
}

/// A statically known list of weighted assessors.
pub trait AssessorSet {
    fn total<L: LedgerSink>(&self, board: &DuelBoard, ledger: &mut L) -> i32;
}

impl AssessorSet for () {
    fn total<L: LedgerSink>(&self, _board: &DuelBoard, _ledger: &mut L) -> i32 {
        0
    }
}

impl<A: Assessor> AssessorSet for Weighted<A> {
    fn total<L: LedgerSink>(&self, board: &DuelBoard, ledger: &mut L) -> i32 {
        let entry = LedgerEntry {
            name: A::NAME,
            raw: self.assessor.assess(board),
            weight: self.weight,
        };
        ledger.record(entry);
        entry.contribution()
    }
}

impl<Rest: AssessorSet, Last: AssessorSet> AssessorSet for (Rest, Last) {
    fn total<L: LedgerSink>(&self, board: &DuelBoard, ledger: &mut L) -> i32 {
        self.0.total(board, ledger) + self.1.total(board, ledger)
    }
}

/// The outcome of assessing one position with a ledger.
#[derive(Clone, Copy, Debug)]
pub struct Valuation {
    pub score: i32,
    pub ledger: Ledger,
}

#[derive(Clone, Copy, Debug)]
pub struct ValuationPipeline<S> {
    terms: S,
}

impl ValuationPipeline<()> {
    #[must_use]
    pub const fn empty() -> Self {
        Self { terms: () }
    }
}

impl<S: AssessorSet> ValuationPipeline<S> {
    /// Registers another weighted assessor after the existing ones.
    #[must_use]
    pub fn with<A: Assessor>(
        self,
        assessor: A,
        weight: i32,
    ) -> ValuationPipeline<(S, Weighted<A>)> {
        ValuationPipeline {
            terms: (self.terms, Weighted { assessor, weight }),
        }
    }

    /// The score alone, without building a ledger.
    #[must_use]
    pub fn score(&self, board: &DuelBoard) -> i32 {
        self.terms.total(board, &mut Unrecorded)
    }

    /// The score with the full per-term ledger.
    #[must_use]
    pub fn assess(&self, board: &DuelBoard) -> Valuation {
        let mut ledger = Ledger::new();
        let score = self.terms.total(board, &mut ledger);
        Valuation { score, ledger }
    }
}
