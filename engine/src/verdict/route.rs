//! Which engine answers a classified request.

use crate::arena::duel::DuelBoard;
use crate::arena::ingest::ingest;
use crate::rules_core::{FallbackContext, Scope, TurnState};

/// The engine a request is handed to, with exactly the data that engine needs,
/// borrowed from the classified scope so nothing is copied or converted twice.
#[derive(Debug)]
pub enum Route<'scope> {
    /// Exactly two snakes inside the certified scope: time-bounded duel search
    /// over the ingested board. The state stays available for the one-turn
    /// safety fallback should the search complete no depth.
    DuelSearch {
        state: &'scope TurnState,
        board: Box<DuelBoard>,
    },
    /// Three or four snakes inside the certified scope: the reused one-turn
    /// safety engine, unchanged.
    SafetyFallback(&'scope TurnState),
    /// Outside the certified scope: the reused best-effort fallback, unchanged.
    UnsupportedFallback(&'scope FallbackContext),
}

/// Picks the [`Route`] for a request that has already been classified. Pure: the
/// same scope always gets the same route, and nothing is read but the scope.
#[derive(Clone, Copy, Debug, Default)]
pub struct RouteSelector;

impl RouteSelector {
    #[must_use]
    pub fn select(scope: &Scope) -> Route<'_> {
        match scope {
            Scope::Unsupported(context) => Route::UnsupportedFallback(context),
            // Ingest accepts exactly the duels, so three and four snakes land here
            // as `NotADuel`, and so would a two-snake state the kernel refuses
            // (the reused classifier rules those out, but the safety engine is
            // the right answer for one either way).
            Scope::Supported(state) => {
                ingest(state).map_or(Route::SafetyFallback(state), |board| Route::DuelSearch {
                    state,
                    board: Box::new(board),
                })
            }
        }
    }
}
