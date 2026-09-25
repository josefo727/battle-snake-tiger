//! What one move decision was and how it was reached, as plain data.

use crate::rules_core::{DecisionReport, Direction, SafetyDiagnostic};

/// Which engine produced the move (`engine_path` in the diagnostics schema).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnginePath {
    DuelSearch,
    MeleeSearch,
    SafetyFallback,
    UnsupportedFallback,
}

/// Why that move was chosen (`selection_reason` in the diagnostics schema).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectionReason {
    /// The move of the deepest fully completed search depth.
    SearchCompletedDepth,
    /// The search proved a forced win.
    SearchTerminalWin,
    /// The search proved a forced loss under its paranoid opponent model, so
    /// the move was taken from the rollouts instead (growth iteration 20).
    SearchLostSoRolloutsChose,
    /// The duel search could not finish even one depth, so the one-turn safety
    /// engine answered.
    BudgetExhaustedBeforeFirstDepth,
    OneTurnSafety,
    UnsupportedBestEffort,
}

/// Whether the decision needs operator attention (`diagnostic` in the schema).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Diagnostic {
    None,
    UnsupportedScope,
    DeadlineCutoff,
}

impl From<SafetyDiagnostic> for Diagnostic {
    fn from(reused: SafetyDiagnostic) -> Self {
        match reused {
            SafetyDiagnostic::None => Self::None,
            SafetyDiagnostic::UnsupportedScope => Self::UnsupportedScope,
            SafetyDiagnostic::DeadlineCutoff => Self::DeadlineCutoff,
        }
    }
}

/// The complete record of one `/move` decision; the transport turns it into the
/// response body and the diagnostic event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VerdictReport {
    pub engine_path: EnginePath,
    pub selected_move: Direction,
    pub elapsed_us: u64,
    /// Greatest fully completed search depth; zero outside a completed search.
    pub search_depth: u16,
    /// Positions visited across every iteration of this decision.
    pub nodes_explored: u64,
    /// Score of the best line at the completed depth; `None` outside the search.
    pub principal_score: Option<i32>,
    pub fallback_used: bool,
    pub selection_reason: SelectionReason,
    pub diagnostic: Diagnostic,
}

impl VerdictReport {
    /// A decision made by a search (`engine_path` names which) at a completed depth.
    #[must_use]
    pub const fn searched(
        engine_path: EnginePath,
        selected_move: Direction,
        principal_score: i32,
        proven_win: bool,
        search_depth: u16,
        nodes_explored: u64,
        elapsed_us: u64,
    ) -> Self {
        Self {
            engine_path,
            selected_move,
            elapsed_us,
            search_depth,
            nodes_explored,
            principal_score: Some(principal_score),
            fallback_used: false,
            selection_reason: if proven_win {
                SelectionReason::SearchTerminalWin
            } else {
                SelectionReason::SearchCompletedDepth
            },
            diagnostic: Diagnostic::None,
        }
    }

    /// The same, for a position the search proved lost: the move is the one the
    /// rollouts liked, not the one the search named.
    #[must_use]
    pub const fn rescued_by_rollouts(self, selected_move: Direction) -> Self {
        Self {
            selected_move,
            selection_reason: SelectionReason::SearchLostSoRolloutsChose,
            ..self
        }
    }

    /// A search finished no depth, so `reused` (the one-turn safety
    /// decision) answered; the positions the search did visit still count.
    #[must_use]
    pub fn budget_exhausted(reused: &DecisionReport, search_nodes: u64, elapsed_us: u64) -> Self {
        Self {
            engine_path: EnginePath::SafetyFallback,
            selected_move: reused.selected_move,
            elapsed_us,
            search_depth: 0,
            nodes_explored: search_nodes.saturating_add(nodes_visited(reused)),
            principal_score: None,
            fallback_used: true,
            selection_reason: SelectionReason::BudgetExhaustedBeforeFirstDepth,
            diagnostic: Diagnostic::DeadlineCutoff,
        }
    }

    /// A reused fallback decision, carried over field for field.
    #[must_use]
    pub fn reused(
        report: &DecisionReport,
        engine_path: EnginePath,
        selection_reason: SelectionReason,
    ) -> Self {
        Self {
            engine_path,
            selected_move: report.selected_move,
            elapsed_us: report.elapsed_us,
            search_depth: 0,
            nodes_explored: nodes_visited(report),
            principal_score: None,
            fallback_used: report.fallback_used,
            selection_reason,
            diagnostic: report.diagnostic.into(),
        }
    }
}

fn nodes_visited(report: &DecisionReport) -> u64 {
    u64::try_from(report.nodes_explored).unwrap_or(u64::MAX)
}
