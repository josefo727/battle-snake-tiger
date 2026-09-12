//! What a finished search reports.

use crate::arena::heading::Heading;

/// The outcome of one search: the greatest fully completed depth and the move
/// that depth chose, or nothing when no depth completed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LookaheadReport {
    pub completed_depth: u16,
    pub nodes_explored: u64,
    pub best: Option<Heading>,
    pub principal_score: Option<i32>,
}

impl LookaheadReport {
    /// No depth completed before the allowance ran out.
    #[must_use]
    pub const fn nothing_completed(nodes_explored: u64) -> Self {
        Self {
            completed_depth: 0,
            nodes_explored,
            best: None,
            principal_score: None,
        }
    }

    #[must_use]
    pub const fn completed(
        completed_depth: u16,
        nodes_explored: u64,
        best: Heading,
        principal_score: i32,
    ) -> Self {
        Self {
            completed_depth,
            nodes_explored,
            best: Some(best),
            principal_score: Some(principal_score),
        }
    }

    /// Whether at least one depth completed.
    #[must_use]
    pub const fn has_result(&self) -> bool {
        self.best.is_some()
    }
}
