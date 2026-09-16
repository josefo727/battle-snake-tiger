//! Which engine answers a classified request.

use crate::arena::duel::DuelBoard;
use crate::arena::ingest::{ingest, ingest_melee};
use crate::arena::melee::MeleeBoard;
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
    /// Three or four snakes inside the certified scope: time-bounded paranoid
    /// search over the ingested melee, with the same fallback as the duel.
    MeleeSearch {
        state: &'scope TurnState,
        board: Box<MeleeBoard>,
    },
    /// A supported request neither kernel accepts (one snake): the reused
    /// one-turn safety engine, unchanged.
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
            // The duel kernel accepts exactly two snakes and the melee kernel
            // three or four; a supported state neither takes (one snake, or a
            // state a kernel refuses, which the reused classifier rules out)
            // gets the safety engine.
            Scope::Supported(state) => {
                if let Ok(board) = ingest(state) {
                    Route::DuelSearch {
                        state,
                        board: Box::new(board),
                    }
                } else if let Ok(board) = ingest_melee(state) {
                    Route::MeleeSearch {
                        state,
                        board: Box::new(board),
                    }
                } else {
                    Route::SafetyFallback(state)
                }
            }
        }
    }
}
