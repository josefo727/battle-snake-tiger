// contract: rules-core-dependency
// version: 1.0.0
// captured: 2026-09-18
// source: research.md §rules-core rules core@efed780
//
// The exact public surface `tiger-engine` consumes from the sibling crate
// `rules-core`, pinned to commit `efed780` (path dependency
// `../rules-core`). This file is documentation of a typed boundary, not
// compiled code; contract tests in `engine/tests/rules_core_contract.rs`
// import each item below and fail to compile if the sibling drifts.
//
// Consumed for: wire parsing and scope classification, the reference turn
// resolver used by differential tests, the injected clock, and the one-turn
// safety fallback for every non-duel request.

// --- Wire parsing and scope (transport) ---------------------------------
//   rules_core::transport::dto::{TurnRequestDto, classify,
//       to_turn_state, declared_timeout, direction_to_wire, MoveResponseDto}
//   rules_core::application::decision::{Scope, FallbackContext}
//
// Behavior relied on: `classify` returns `Scope::Supported(TurnState)` for
// Standard v1.2.3 / "cli", non-wrapped, hazard-free 11x11 requests with 2-4
// snakes and a timeout above the 120 ms reserve, and `Scope::Unsupported`
// otherwise (never panics).

// --- Reference model (differential tests only) --------------------------
//   rules_core::domain::state::{TurnState, SnakeState}
//   rules_core::domain::simulation::{Direction, JointMoves,
//       TurnResolution, resolve_turn}
//   rules_core::domain::board::{CellIndex, Coordinate, BoardMask}
//
// Behavior relied on: `resolve_turn(&TurnState, &JointMoves)` implements the
// Standard resolution order exactly and agrees with the official rules CLI
// (sibling `tests/rules_conformance.rs`, 8 scenarios).

// --- Time and deadlines --------------------------------------------------
//   rules_core::application::clock::{Clock, MonotonicInstant,
//       RequestTiming, RESPONSE_RESERVE, response_deadline}
//
// --- One-turn safety fallback (spec.md criterion 5) ---------------------
//   rules_core::application::decision::{decide_within_deadline,
//       decide_unsupported, DecisionReport}
//
// Change policy: any sibling commit that alters one of these items requires
// re-running `engine/tests/rules_core_contract.rs` and updating this file's
// pinned commit.
