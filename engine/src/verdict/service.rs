//! The one entry the transport calls: request in, verdict out.

use std::sync::Arc;

use super::report::{EnginePath, SelectionReason, VerdictReport};
use super::route::{Route, RouteSelector};
use crate::arena::duel::DuelBoard;
use crate::arena::ingest::direction_of;
use crate::arena::melee::MeleeBoard;
use crate::lookahead::allowance::SearchAllowance;
use crate::lookahead::deepening::{DEPTH_CEILING, deepen, deepen_melee, drive};
use crate::lookahead::ledger::LookaheadReport;
use crate::lookahead::minimax::Searcher;
use crate::lookahead::parallel::RootSplit;
use crate::lookahead::paranoid::{self, MeleeSearcher};
use crate::lookahead::rollout;
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
    /// Threads of the melee search's root split; one means no split.
    threads: usize,
    /// What a root heading pays for a cell an equal-length rival can enter.
    trade_risk: i32,
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
            threads: 1,
            trade_risk: paranoid::TRADE_RISK,
        }
    }

    /// What a root heading pays for a cell an equal-length rival can also
    /// enter. Zero makes a sparring opponent that never declines the
    /// head-to-head, which is the only way the local bench can charge for one.
    #[must_use]
    pub const fn with_trade_risk(mut self, trade_risk: i32) -> Self {
        self.trade_risk = trade_risk;
        self
    }

    /// Splits the melee search at the root over `threads` threads (at least one).
    #[must_use]
    pub const fn with_threads(mut self, threads: usize) -> Self {
        self.threads = if threads == 0 { 1 } else { threads };
        self
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
        let searched = if self.threads > 1 {
            let mut split = RootSplit::new(
                &self.melee_valuation,
                self.melee_finish,
                *board,
                self.clock.as_ref(),
                allowance.deadline(),
                self.threads,
            );
            drive(&mut split, &mut allowance, self.depth_limit)
        } else {
            let mut searcher = MeleeSearcher::new(&self.melee_valuation, self.melee_finish)
                .with_trade_risk(self.trade_risk);
            deepen_melee(&mut searcher, board, &mut allowance, self.depth_limit)
        };
        let finish = self.melee_finish;
        let decisive = move |score: i32| score.abs() >= finish.finite_limit();
        let report = self.report_search(
            EnginePath::MeleeSearch,
            &searched,
            decisive,
            request,
            state,
            arrived_at,
        );
        self.rescue_a_lost_melee(board, &searched, report, &mut allowance)
    }

    /// When the search proved the melee lost, the move it names is whichever
    /// heading the fixed order reached first among several it scored the same
    /// kind of nothing. The proof holds only against an opponent model in which
    /// all three rivals play the worst line for us every turn, and they do not,
    /// so the rollouts get the last word with whatever is left of the budget.
    ///
    /// Anything short of a proof is left alone, and so is a rescue that runs
    /// out of time: half a set of rollouts is not an answer.
    fn rescue_a_lost_melee(
        &self,
        board: &MeleeBoard,
        searched: &LookaheadReport,
        report: VerdictReport,
        allowance: &mut SearchAllowance<'_>,
    ) -> VerdictReport {
        let lost = searched
            .principal_score
            .is_some_and(|score| score < 0 && score.abs() >= self.melee_finish.finite_limit());
        if !lost {
            return report;
        }
        rollout::best_heading(board, allowance).map_or(report, |heading| {
            report.rescued_by_rollouts(direction_of(heading))
        })
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
