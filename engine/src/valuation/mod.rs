//! Position assessment: a pipeline of single-question assessors combined by an
//! explicit weight sheet. Everything here is integer arithmetic and statically
//! dispatched, so scoring a leaf never allocates or goes through a vtable.

pub mod dominion;
pub mod enclosure;
pub mod fill;
pub mod finish;
pub mod leverage;
pub mod melee;
pub mod sustenance;
pub mod weights;

use self::dominion::Dominion;
use self::enclosure::Enclosure;
use self::leverage::{HeadPressure, LengthAdvantage};
use self::sustenance::Sustenance;
use self::weights::{DEFAULT_PROFILE, WeightSheet};
use crate::arena::duel::DuelBoard;

/// Most terms one ledger can describe.
pub const LEDGER_CAPACITY: usize = 9;

/// One assessor answering one question about a position of its board type
/// (a duel or a melee).
pub trait Assessor {
    /// The kind of position this assessor reads.
    type Board;

    /// Name reported in the ledger.
    const NAME: &'static str;

    /// A safe upper bound on the magnitude of [`Assessor::assess`], so a
    /// pipeline can prove its scores stay inside the terminal-score limits.
    const MAX_RAW: i32;

    /// The raw, unweighted answer from our point of view: positive favours us.
    fn assess(&self, board: &Self::Board) -> i32;
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

/// The largest magnitude a list of weighted assessors could ever total: the
/// sum of each term's weight times its `MAX_RAW`. Independent of any board.
pub trait Bounded {
    fn worst_case(&self) -> i32;
}

/// A statically known list of weighted assessors of one board type.
pub trait AssessorSet<B = DuelBoard>: Bounded {
    fn total<L: LedgerSink>(&self, board: &B, ledger: &mut L) -> i32;
}

impl Bounded for () {
    fn worst_case(&self) -> i32 {
        0
    }
}

impl<B> AssessorSet<B> for () {
    fn total<L: LedgerSink>(&self, _board: &B, _ledger: &mut L) -> i32 {
        0
    }
}

impl<A: Assessor> Bounded for Weighted<A> {
    fn worst_case(&self) -> i32 {
        self.weight.abs() * A::MAX_RAW
    }
}

impl<A: Assessor> AssessorSet<A::Board> for Weighted<A> {
    fn total<L: LedgerSink>(&self, board: &A::Board, ledger: &mut L) -> i32 {
        let entry = LedgerEntry {
            name: A::NAME,
            raw: self.assessor.assess(board),
            weight: self.weight,
        };
        debug_assert!(
            entry.raw.abs() <= A::MAX_RAW,
            "{} produced {} beyond its declared bound {}",
            A::NAME,
            entry.raw,
            A::MAX_RAW
        );
        ledger.record(entry);
        entry.contribution()
    }
}

impl<Rest: Bounded, Last: Bounded> Bounded for (Rest, Last) {
    fn worst_case(&self) -> i32 {
        self.0.worst_case() + self.1.worst_case()
    }
}

impl<B, Rest: AssessorSet<B>, Last: AssessorSet<B>> AssessorSet<B> for (Rest, Last) {
    fn total<L: LedgerSink>(&self, board: &B, ledger: &mut L) -> i32 {
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

impl<S: Bounded> ValuationPipeline<S> {
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

    /// The largest magnitude any score from this pipeline could have.
    #[must_use]
    pub fn worst_case_magnitude(&self) -> i32 {
        self.terms.worst_case()
    }

    /// The score alone, without building a ledger.
    #[must_use]
    pub fn score<B>(&self, board: &B) -> i32
    where
        S: AssessorSet<B>,
    {
        self.terms.total(board, &mut Unrecorded)
    }

    /// The score with the full per-term ledger.
    #[must_use]
    pub fn assess<B>(&self, board: &B) -> Valuation
    where
        S: AssessorSet<B>,
    {
        let mut ledger = Ledger::new();
        let score = self.terms.total(board, &mut ledger);
        Valuation { score, ledger }
    }
}

/// The five positional terms of the standard valuation, in ledger order.
pub type StandardTerms = (
    (
        (
            (((), Weighted<Dominion>), Weighted<Sustenance>),
            Weighted<LengthAdvantage>,
        ),
        Weighted<HeadPressure>,
    ),
    Weighted<Enclosure>,
);

pub type StandardPipeline = ValuationPipeline<StandardTerms>;

impl StandardPipeline {
    /// The valuation search uses, with [`DEFAULT_PROFILE`].
    #[must_use]
    pub fn standard() -> Self {
        Self::with_profile(&DEFAULT_PROFILE)
    }

    /// The same five terms with the coefficients of `weights`, for experiments.
    #[must_use]
    pub fn with_profile(weights: &WeightSheet) -> Self {
        ValuationPipeline::empty()
            .with(Dominion, weights.territory_cell)
            .with(Sustenance, weights.hunger_urgency)
            .with(LengthAdvantage, weights.length_advantage)
            .with(HeadPressure, weights.head_pressure)
            .with(Enclosure, weights.enclosure_turn)
    }
}
