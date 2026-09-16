//! The diagnostics port: one `move_decision` event per decision.
//!
//! The event is the flat shape of `contracts/decision-diagnostic.schema.json`
//! (schema 2.1.0: 2.0.0 plus the `melee_search` engine path). It carries the game id and turn and the numbers that describe
//! how the decision was reached, and never a board, a body, a name or a shout.

use serde::Serialize;

use crate::rules_core::direction_to_wire;
use crate::verdict::report::{Diagnostic, EnginePath, SelectionReason, VerdictReport};

pub const SCHEMA_VERSION: &str = "2.1.0";

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct DecisionEvent {
    pub level: &'static str,
    pub target: &'static str,
    pub event: &'static str,
    pub schema_version: &'static str,
    pub game_id: String,
    pub turn: u64,
    pub engine_path: &'static str,
    pub elapsed_us: u64,
    pub search_depth: u16,
    pub nodes_explored: u64,
    pub principal_score: Option<i32>,
    pub fallback_used: bool,
    pub selected_move: &'static str,
    pub selection_reason: &'static str,
    pub diagnostic: &'static str,
}

impl DecisionEvent {
    /// The event for one decision. A negative turn (which the platform never
    /// sends) becomes 0 and an empty game id becomes "unknown", so every event
    /// satisfies the schema whatever the request carried.
    #[must_use]
    pub fn new(report: &VerdictReport, game_id: &str, turn: i64) -> Self {
        Self {
            level: if report.diagnostic == Diagnostic::None {
                "INFO"
            } else {
                "WARN"
            },
            target: "move_decision",
            event: "move_decision",
            schema_version: SCHEMA_VERSION,
            game_id: if game_id.is_empty() {
                "unknown"
            } else {
                game_id
            }
            .to_owned(),
            turn: u64::try_from(turn).unwrap_or(0),
            engine_path: engine_path_name(report.engine_path),
            elapsed_us: report.elapsed_us,
            search_depth: report.search_depth,
            nodes_explored: report.nodes_explored,
            principal_score: report.principal_score,
            fallback_used: report.fallback_used,
            selected_move: direction_to_wire(report.selected_move),
            selection_reason: reason_name(report.selection_reason),
            diagnostic: diagnostic_name(report.diagnostic),
        }
    }
}

const fn engine_path_name(path: EnginePath) -> &'static str {
    match path {
        EnginePath::DuelSearch => "duel_search",
        EnginePath::MeleeSearch => "melee_search",
        EnginePath::SafetyFallback => "safety_fallback",
        EnginePath::UnsupportedFallback => "unsupported_fallback",
    }
}

const fn reason_name(reason: SelectionReason) -> &'static str {
    match reason {
        SelectionReason::SearchCompletedDepth => "search_completed_depth",
        SelectionReason::SearchTerminalWin => "search_terminal_win",
        SelectionReason::BudgetExhaustedBeforeFirstDepth => "budget_exhausted_before_first_depth",
        SelectionReason::OneTurnSafety => "one_turn_safety",
        SelectionReason::UnsupportedBestEffort => "unsupported_best_effort",
    }
}

const fn diagnostic_name(diagnostic: Diagnostic) -> &'static str {
    match diagnostic {
        Diagnostic::None => "none",
        Diagnostic::UnsupportedScope => "unsupported_scope",
        Diagnostic::DeadlineCutoff => "deadline_cutoff",
    }
}

/// Where decision events go. A failure to record must never change the move or
/// the response, so `emit` returns nothing to propagate.
pub trait DecisionBeacon: Send + Sync {
    fn emit(&self, event: &DecisionEvent);
}

/// The production beacon: one `tracing` event per decision, at `INFO`, or `WARN`
/// when the decision needs attention. Formatting is the binary's business.
#[derive(Clone, Copy, Debug, Default)]
pub struct TracingBeacon;

impl DecisionBeacon for TracingBeacon {
    fn emit(&self, event: &DecisionEvent) {
        let payload = serde_json::to_string(event).unwrap_or_default();
        if event.level == "WARN" {
            tracing::warn!(target: "move_decision", "{payload}");
        } else {
            tracing::info!(target: "move_decision", "{payload}");
        }
    }
}
