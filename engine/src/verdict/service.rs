//! The one entry the transport calls: request in, verdict out.

use std::sync::Arc;

use super::report::{EnginePath, SelectionReason, VerdictReport};
use super::route::{Route, RouteSelector};
use crate::arena::duel::DuelBoard;
use crate::arena::ingest::direction_of;
use crate::arena::melee::MeleeBoard;
use crate::lookahead::allowance::SearchAllowance;
use crate::lookahead::deepening::{DEPTH_CEILING, deepen, deepen_melee};
use crate::lookahead::ledger::LookaheadReport;
use crate::lookahead::minimax::Searcher;
use crate::lookahead::paranoid::MeleeSearcher;
use crate::rules_core::{
    Clock, DecisionReport, MonotonicInstant, RequestTiming, TurnRequestDto, TurnState, classify,
    decide_unsupported, decide_within_deadline, declared_timeout, response_deadline,
};
use crate::valuation::StandardPipeline;
use crate::valuation::finish::Finish;
use crate::valuation::melee::MeleeValuation;
use crate::valuation::melee::finish::MeleeFinish;
use crate::valuation::melee::weights::DEFAULT_MELEE_PROFILE;
use crate::valuation::weights::DEFAULT_PROFILE;

pub struct VerdictService {
    clock: Arc<dyn Clock>,
    pipeline: StandardPipeline,
    finish: Finish,
    melee_valuation: MeleeValuation,
    melee_finish: MeleeFinish,
    depth_limit: u16,
}

impl VerdictService {
    #[must_use]
    pub fn new(clock: Arc<dyn Clock>) -> Self {
        Self {
            clock,
            pipeline: StandardPipeline::standard(),
            finish: Finish::new(&DEFAULT_PROFILE),
            melee_valuation: MeleeValuation::standard(),
            melee_finish: MeleeFinish::new(&DEFAULT_MELEE_PROFILE),
            depth_limit: DEPTH_CEILING,
        }
    }

    /// Caps the search depth below the ceiling (tests and sparring use this to
    /// keep runs short and repeatable).
    #[must_use]
    pub const fn with_depth_limit(mut self, depth_limit: u16) -> Self {
        self.depth_limit = depth_limit;
        self
    }

    /// Decides the move for `request`, which reached the server at `arrived_at`.
    ///
    /// # Panics
    ///
    /// Panics only if the reused safety engine rejects a state its own
    /// classifier validated, which it cannot.
    #[must_use]
    pub fn decide(&self, request: &TurnRequestDto, arrived_at: MonotonicInstant) -> VerdictReport {
        let scope = classify(request);
        match RouteSelector::select(&scope) {
            Route::DuelSearch { state, board } => self.duel(request, state, &board, arrived_at),
            Route::MeleeSearch { state, board } => self.melee(request, state, &board, arrived_at),
            Route::SafetyFallback(state) => {
                let report = self.safety_decision(request, state, arrived_at);
                VerdictReport::reused(
                    &report,
                    EnginePath::SafetyFallback,
                    SelectionReason::OneTurnSafety,
                )
            }
            Route::UnsupportedFallback(context) => {
                let report = decide_unsupported(context, self.clock.as_ref(), arrived_at);
                VerdictReport::reused(
                    &report,
                    EnginePath::UnsupportedFallback,
                    SelectionReason::UnsupportedBestEffort,
                )
            }
        }
    }

    fn duel(
        &self,
        request: &TurnRequestDto,
        state: &TurnState,
        board: &DuelBoard,
        arrived_at: MonotonicInstant,
    ) -> VerdictReport {
        let mut allowance = self.allowance_for(request, arrived_at);
        let mut searcher = Searcher::new(&self.pipeline, self.finish);
        let searched = deepen(&mut searcher, board, &mut allowance, self.depth_limit);
        let decisive = |score| searcher.is_decisive(score);
        self.report_search(
            EnginePath::DuelSearch,
            &searched,
            decisive,
            request,
            state,
            arrived_at,
        )
    }

    fn melee(
        &self,
        request: &TurnRequestDto,
        state: &TurnState,
        board: &MeleeBoard,
        arrived_at: MonotonicInstant,
    ) -> VerdictReport {
        let mut allowance = self.allowance_for(request, arrived_at);
        let mut searcher = MeleeSearcher::new(&self.melee_valuation, self.melee_finish);
        let searched = deepen_melee(&mut searcher, board, &mut allowance, self.depth_limit);
        let decisive = |score| searcher.is_decisive(score);
        self.report_search(
            EnginePath::MeleeSearch,
            &searched,
            decisive,
            request,
            state,
            arrived_at,
        )
    }

    /// The report of a deepening search: its move at the completed depth, or the
    /// reused one-turn safety decision when no depth completed.
    fn report_search(
        &self,
        engine_path: EnginePath,
        searched: &LookaheadReport,
        is_decisive: impl Fn(i32) -> bool,
        request: &TurnRequestDto,
        state: &TurnState,
        arrived_at: MonotonicInstant,
    ) -> VerdictReport {
        let (Some(best), Some(score)) = (searched.best, searched.principal_score) else {
            let reused = self.safety_decision(request, state, arrived_at);
            return VerdictReport::budget_exhausted(
                &reused,
                searched.nodes_explored,
                self.elapsed_since(arrived_at),
            );
        };

        VerdictReport::searched(
            engine_path,
            direction_of(best),
            score,
            score > 0 && is_decisive(score),
            searched.completed_depth,
            searched.nodes_explored,
            self.elapsed_since(arrived_at),
        )
    }

    fn allowance_for(
        &self,
        request: &TurnRequestDto,
        arrived_at: MonotonicInstant,
    ) -> SearchAllowance<'_> {
        SearchAllowance::new(
            self.clock.as_ref(),
            Self::response_deadline_of(request, arrived_at),
        )
    }

    fn safety_decision(
        &self,
        request: &TurnRequestDto,
        state: &TurnState,
        arrived_at: MonotonicInstant,
    ) -> DecisionReport {
        decide_within_deadline(
            state,
            self.clock.as_ref(),
            arrived_at,
            Self::response_deadline_of(request, arrived_at),
        )
        .expect("a validated TurnState always resolves")
    }

    fn response_deadline_of(
        request: &TurnRequestDto,
        arrived_at: MonotonicInstant,
    ) -> MonotonicInstant {
        response_deadline(RequestTiming { arrived_at }, declared_timeout(request))
            .expect("a supported scope guarantees a timeout above the reserve")
    }

    fn elapsed_since(&self, arrived_at: MonotonicInstant) -> u64 {
        self.clock
            .now()
            .microseconds
            .saturating_sub(arrived_at.microseconds)
    }
}
