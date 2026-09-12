mod support;

use tiger_engine::rules_core::{
    BoardMask, CellIndex, Clock, Coordinate, DecisionReport, Direction, FallbackContext,
    JointMoves, MonotonicInstant, MoveResponseDto, RESPONSE_RESERVE, RequestTiming,
    SafetyDiagnostic, Scope, SnakeState, TurnRequestDto, TurnResolution, TurnState, classify,
    decide_unsupported, decide_within_deadline, declared_timeout, direction_to_wire, resolve_turn,
    response_deadline, supported_snake_count, to_turn_state,
};

use support::request;

#[test]
fn every_contract_item_is_reachable_through_the_facade() {
    // Compile-time contract: each item named by contracts/rules-core-dependency.rs
    // must resolve through `rules_core` with the shape the engine relies on.
    let _classify: fn(&TurnRequestDto) -> Scope = classify;
    let _to_turn_state = to_turn_state;
    let _declared_timeout = declared_timeout;
    let _wire: fn(Direction) -> &'static str = direction_to_wire;
    let _resolve: fn(&TurnState, &JointMoves) -> _ = resolve_turn;
    let _decide_unsupported = decide_unsupported;
    let _decide_within_deadline = decide_within_deadline;
    let _response_deadline = response_deadline;
    let _reserve = RESPONSE_RESERVE;

    fn assert_clock<T: Clock + ?Sized>() {}
    assert_clock::<dyn Clock>();

    fn assert_type<T>() {}
    assert_type::<BoardMask>();
    assert_type::<CellIndex>();
    assert_type::<Coordinate>();
    assert_type::<DecisionReport>();
    assert_type::<FallbackContext>();
    assert_type::<MonotonicInstant>();
    assert_type::<MoveResponseDto>();
    assert_type::<RequestTiming>();
    assert_type::<SafetyDiagnostic>();
    assert_type::<SnakeState>();
    assert_type::<TurnResolution>();
}

#[test]
fn a_production_duel_classifies_as_supported_with_two_snakes() {
    let scope = classify(&request("v1.2.3", 2, 500));

    assert_eq!(supported_snake_count(&scope), Some(2));
}

#[test]
fn a_local_cli_duel_classifies_as_supported_with_two_snakes() {
    let scope = classify(&request("cli", 2, 500));

    assert_eq!(supported_snake_count(&scope), Some(2));
}

#[test]
fn a_three_snake_game_reports_three_snakes() {
    let scope = classify(&request("v1.2.3", 3, 500));

    assert_eq!(supported_snake_count(&scope), Some(3));
}

#[test]
fn an_unsupported_version_has_no_supported_snake_count() {
    let scope = classify(&request("v1.0.0", 2, 500));

    assert_eq!(supported_snake_count(&scope), None);
}

#[test]
fn a_timeout_at_the_reserve_has_no_supported_snake_count() {
    let scope = classify(&request("v1.2.3", 2, 120));

    assert_eq!(supported_snake_count(&scope), None);
}
