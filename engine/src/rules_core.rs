//! The single module that names the sibling `battle-snake-rust` crate.
//!
//! Every other module imports rules-core items from here, so the dependency's
//! surface is exactly what `contracts/rules-core-dependency.rs` lists and a
//! sibling change surfaces in one place.

pub use battle_snake_rust::application::clock::{
    Clock, MonotonicInstant, RESPONSE_RESERVE, RequestTiming, response_deadline,
};
pub use battle_snake_rust::application::decision::{
    DecisionReport, Diagnostic as SafetyDiagnostic, FallbackContext, Scope, decide_unsupported,
    decide_within_deadline,
};
pub use battle_snake_rust::domain::board::{BoardMask, CellIndex, Coordinate};
pub use battle_snake_rust::domain::simulation::{
    Direction, JointMoves, TurnResolution, resolve_turn,
};
pub use battle_snake_rust::domain::state::{SnakeState, TurnState};
pub use battle_snake_rust::transport::dto::{
    MoveResponseDto, TurnRequestDto, classify, declared_timeout, direction_to_wire, to_turn_state,
};

/// The number of snakes in a request the reused predicate certifies, or
/// `None` when the request is outside the certified scope.
#[must_use]
pub fn supported_snake_count(scope: &Scope) -> Option<usize> {
    match scope {
        Scope::Supported(state) => Some(state.snakes().len()),
        Scope::Unsupported(_) => None,
    }
}
