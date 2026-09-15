mod support;

use std::sync::Arc;

use proptest::prelude::*;
use serde_json::{Value, json};
use tiger_engine::gateway::beacon::{DecisionBeacon, DecisionEvent, SCHEMA_VERSION, TracingBeacon};
use tiger_engine::rules_core::{Direction, MonotonicInstant};
use tiger_engine::verdict::report::{Diagnostic, EnginePath, SelectionReason, VerdictReport};
use tiger_engine::verdict::service::VerdictService;

use support::capture::json_lines;
use support::clock::ManualClock;
use support::schema::{load_contract, validate};
use support::{request_from_bodies, request_with};

fn schema() -> Value {
    load_contract("decision-diagnostic.schema.json")
}

fn report() -> VerdictReport {
    VerdictReport {
        engine_path: EnginePath::DuelSearch,
        selected_move: Direction::Left,
        elapsed_us: 12_345,
        search_depth: 7,
        nodes_explored: 9_876_543_210,
        principal_score: Some(-321),
        fallback_used: false,
        selection_reason: SelectionReason::SearchCompletedDepth,
        diagnostic: Diagnostic::None,
    }
}

fn as_json(event: &DecisionEvent) -> Value {
    serde_json::to_value(event).expect("an event serializes")
}

// ---- the mapping -------------------------------------------------------------

#[test]
fn search_depth_and_nodes_equal_the_reports_values() {
    let event = DecisionEvent::new(&report(), "game-1", 42);

    assert_eq!(event.search_depth, 7);
    assert_eq!(event.nodes_explored, 9_876_543_210);
    assert_eq!(event.elapsed_us, 12_345);
    assert_eq!(event.principal_score, Some(-321));
}

#[test]
fn the_event_names_the_game_and_turn_and_the_fixed_identity_fields() {
    let event = DecisionEvent::new(&report(), "game-1", 42);

    assert_eq!(event.game_id, "game-1");
    assert_eq!(event.turn, 42);
    assert_eq!(event.target, "move_decision");
    assert_eq!(event.event, "move_decision");
    assert_eq!(event.schema_version, SCHEMA_VERSION);
    assert_eq!(SCHEMA_VERSION, "2.0.0");
}

#[test]
fn every_direction_is_a_platform_move_string() {
    let expected = [
        (Direction::Up, "up"),
        (Direction::Right, "right"),
        (Direction::Down, "down"),
        (Direction::Left, "left"),
    ];

    for (direction, wire) in expected {
        let event = DecisionEvent::new(
            &VerdictReport {
                selected_move: direction,
                ..report()
            },
            "g",
            0,
        );

        assert_eq!(event.selected_move, wire);
    }
}

#[test]
fn every_path_reason_and_diagnostic_has_its_schema_name() {
    let paths = [
        (EnginePath::DuelSearch, "duel_search"),
        (EnginePath::SafetyFallback, "safety_fallback"),
        (EnginePath::UnsupportedFallback, "unsupported_fallback"),
    ];
    let reasons = [
        (
            SelectionReason::SearchCompletedDepth,
            "search_completed_depth",
        ),
        (SelectionReason::SearchTerminalWin, "search_terminal_win"),
        (
            SelectionReason::BudgetExhaustedBeforeFirstDepth,
            "budget_exhausted_before_first_depth",
        ),
        (SelectionReason::OneTurnSafety, "one_turn_safety"),
        (
            SelectionReason::UnsupportedBestEffort,
            "unsupported_best_effort",
        ),
    ];
    let diagnostics = [
        (Diagnostic::None, "none"),
        (Diagnostic::UnsupportedScope, "unsupported_scope"),
        (Diagnostic::DeadlineCutoff, "deadline_cutoff"),
    ];

    for (path, wire) in paths {
        let event = DecisionEvent::new(
            &VerdictReport {
                engine_path: path,
                ..report()
            },
            "g",
            0,
        );
        assert_eq!(event.engine_path, wire);
    }
    for (reason, wire) in reasons {
        let event = DecisionEvent::new(
            &VerdictReport {
                selection_reason: reason,
                ..report()
            },
            "g",
            0,
        );
        assert_eq!(event.selection_reason, wire);
    }
    for (diagnostic, wire) in diagnostics {
        let event = DecisionEvent::new(
            &VerdictReport {
                diagnostic,
                ..report()
            },
            "g",
            0,
        );
        assert_eq!(event.diagnostic, wire);
    }
}

#[test]
fn the_level_is_info_unless_the_decision_needs_attention() {
    for (diagnostic, level) in [
        (Diagnostic::None, "INFO"),
        (Diagnostic::UnsupportedScope, "WARN"),
        (Diagnostic::DeadlineCutoff, "WARN"),
    ] {
        let event = DecisionEvent::new(
            &VerdictReport {
                diagnostic,
                ..report()
            },
            "g",
            0,
        );

        assert_eq!(event.level, level, "{diagnostic:?}");
    }
}

#[test]
fn the_principal_score_is_a_number_in_a_search_and_an_explicit_null_outside_it() {
    let searched = as_json(&DecisionEvent::new(&report(), "g", 0));
    let fallback = as_json(&DecisionEvent::new(
        &VerdictReport {
            engine_path: EnginePath::SafetyFallback,
            principal_score: None,
            ..report()
        },
        "g",
        0,
    ));

    assert_eq!(searched["principal_score"], json!(-321));
    assert_eq!(fallback["principal_score"], Value::Null);
}

#[test]
fn a_negative_turn_or_an_empty_game_id_still_yields_a_valid_event() {
    let event = DecisionEvent::new(&report(), "", -5);

    assert_eq!(event.turn, 0);
    assert_eq!(event.game_id, "unknown");
    validate(&schema(), &as_json(&event)).expect("still valid");
}

// ---- the schema ----------------------------------------------------------------

#[test]
fn the_event_members_are_exactly_the_schemas_properties() {
    let schema = schema();
    let json = as_json(&DecisionEvent::new(&report(), "g", 1));

    let mut keys: Vec<&str> = json
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    let mut declared: Vec<&str> = schema["properties"]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    declared.sort_unstable();

    assert_eq!(
        keys, declared,
        "no board, body, name or shout can ride along"
    );
}

#[test]
fn the_schema_check_itself_rejects_a_broken_event() {
    let good = as_json(&DecisionEvent::new(&report(), "g", 1));
    let mut missing = good.clone();
    missing.as_object_mut().unwrap().remove("engine_path");
    let mut extra = good.clone();
    extra["board"] = json!([]);
    let mut wrong_enum = good.clone();
    wrong_enum["selection_reason"] = json!("guess");
    let mut negative = good.clone();
    negative["turn"] = json!(-1);
    let mut wrong_type = good.clone();
    wrong_type["search_depth"] = json!("deep");

    assert!(validate(&schema(), &good).is_ok());
    for (label, broken) in [
        ("missing member", missing),
        ("extra member", extra),
        ("bad enum", wrong_enum),
        ("negative turn", negative),
        ("wrong type", wrong_type),
    ] {
        assert!(
            validate(&schema(), &broken).is_err(),
            "{label} must be rejected"
        );
    }
}

fn any_report() -> impl Strategy<Value = VerdictReport> {
    (
        prop_oneof![
            Just(EnginePath::DuelSearch),
            Just(EnginePath::SafetyFallback),
            Just(EnginePath::UnsupportedFallback),
        ],
        prop_oneof![
            Just(Direction::Up),
            Just(Direction::Right),
            Just(Direction::Down),
            Just(Direction::Left),
        ],
        (
            any::<u64>(),
            any::<u16>(),
            any::<u64>(),
            proptest::option::of(any::<i32>()),
        ),
        any::<bool>(),
        prop_oneof![
            Just(SelectionReason::SearchCompletedDepth),
            Just(SelectionReason::SearchTerminalWin),
            Just(SelectionReason::BudgetExhaustedBeforeFirstDepth),
            Just(SelectionReason::OneTurnSafety),
            Just(SelectionReason::UnsupportedBestEffort),
        ],
        prop_oneof![
            Just(Diagnostic::None),
            Just(Diagnostic::UnsupportedScope),
            Just(Diagnostic::DeadlineCutoff),
        ],
    )
        .prop_map(
            |(
                engine_path,
                selected_move,
                (elapsed_us, search_depth, nodes, score),
                fallback,
                reason,
                diagnostic,
            )| {
                VerdictReport {
                    engine_path,
                    selected_move,
                    elapsed_us,
                    search_depth,
                    nodes_explored: nodes,
                    principal_score: score,
                    fallback_used: fallback,
                    selection_reason: reason,
                    diagnostic,
                }
            },
        )
}

proptest! {
    #[test]
    fn any_report_produces_an_event_that_validates_against_the_schema(
        report in any_report(),
        game_id in ".{0,24}",
        turn in any::<i64>(),
    ) {
        let event = DecisionEvent::new(&report, &game_id, turn);

        prop_assert_eq!(validate(&schema(), &as_json(&event)), Ok(()));
        prop_assert_eq!(event.search_depth, report.search_depth);
        prop_assert_eq!(event.nodes_explored, report.nodes_explored);
    }
}

// ---- real decisions on every engine path --------------------------------------

fn decide_event(request: &tiger_engine::rules_core::TurnRequestDto) -> DecisionEvent {
    let clock = Arc::new(ManualClock::at_micros(1_000_000));
    let arrived = MonotonicInstant {
        microseconds: 1_000_000,
    };
    let report = VerdictService::new(clock)
        .with_depth_limit(2)
        .decide(request, arrived);
    DecisionEvent::new(&report, &request.game.id, request.turn)
}

#[test]
fn a_duel_a_melee_and_an_unsupported_game_each_produce_a_valid_event() {
    let duel = request_from_bodies(
        &[&[(4, 5), (3, 5), (2, 5)], &[(7, 5), (8, 5), (9, 5)]],
        &[90, 90],
        0,
        &[(5, 8)],
    );
    let melee = request_with("v1.2.3", 3, 500, 0, &[]);
    let unsupported = request_with("v9.9.9", 2, 500, 0, &[]);

    for (label, request, path) in [
        ("duel", duel, "duel_search"),
        ("melee", melee, "safety_fallback"),
        ("unsupported", unsupported, "unsupported_fallback"),
    ] {
        let event = decide_event(&request);
        let json = as_json(&event);

        assert_eq!(validate(&schema(), &json), Ok(()), "{label}: {json}");
        assert_eq!(event.engine_path, path, "{label}");
        assert_eq!(event.game_id, "test-game", "{label}");
        let text = json.to_string();
        assert!(
            !text.contains("snake-") && !text.contains("body"),
            "{label}: the event must not carry snake data: {text}"
        );
    }
}

// ---- the tracing adapter --------------------------------------------------------

fn captured_lines(events: &[DecisionEvent]) -> Vec<Value> {
    json_lines(|| {
        for event in events {
            TracingBeacon.emit(event);
        }
    })
}

#[test]
fn the_tracing_beacon_emits_exactly_one_event_per_decision_with_the_events_json() {
    let info = DecisionEvent::new(&report(), "game-1", 3);
    let warn = DecisionEvent::new(
        &VerdictReport {
            diagnostic: Diagnostic::DeadlineCutoff,
            engine_path: EnginePath::SafetyFallback,
            selection_reason: SelectionReason::BudgetExhaustedBeforeFirstDepth,
            principal_score: None,
            fallback_used: true,
            ..report()
        },
        "game-1",
        4,
    );

    let lines = captured_lines(&[info.clone(), warn.clone()]);

    assert_eq!(lines.len(), 2);
    for (line, event) in lines.iter().zip([&info, &warn]) {
        assert_eq!(line["target"], "move_decision");
        assert_eq!(line["level"], event.level);
        let payload: Value =
            serde_json::from_str(line["fields"]["message"].as_str().expect("a message")).unwrap();
        assert_eq!(payload, as_json(event));
        assert_eq!(validate(&schema(), &payload), Ok(()));
    }
}

#[test]
fn a_beacon_is_usable_as_a_shared_trait_object() {
    let beacon: Arc<dyn DecisionBeacon> = Arc::new(TracingBeacon);

    beacon.emit(&DecisionEvent::new(&report(), "g", 0));
}
