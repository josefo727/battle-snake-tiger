mod support;

use proptest::prelude::*;
use serde_json::Value;
use tiger_engine::gateway::lifecycle::{
    LifecycleBeacon, LifecycleEvent, SCHEMA_VERSION, TracingLifecycleBeacon,
};
use tiger_engine::rules_core::TurnRequestDto;

use support::capture::json_lines;
use support::request_with;
use support::schema::validate;

fn contract() -> Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../.specs/002-game-logging/contracts/game-lifecycle.schema.json");
    serde_json::from_str(&std::fs::read_to_string(path).expect("the lifecycle contract exists"))
        .unwrap()
}

fn request(snakes: usize) -> TurnRequestDto {
    request_with("v1.2.3", snakes, 500, 0, &[])
}

fn json(event: &LifecycleEvent) -> Value {
    serde_json::to_value(event).unwrap()
}

#[test]
fn a_started_event_names_the_game_and_its_setting_and_counts_the_snakes() {
    let event = LifecycleEvent::started(&request(4));

    assert_eq!(event.event, "game_started");
    assert_eq!(event.target, "game_lifecycle");
    assert_eq!(event.level, "INFO");
    assert_eq!(event.schema_version, SCHEMA_VERSION);
    assert_eq!(SCHEMA_VERSION, "1.0.0");
    assert_eq!(event.game_id, "test-game");
    assert_eq!(event.ruleset.as_deref(), Some("standard/v1.2.3"));
    assert_eq!(event.map.as_deref(), Some("standard"));
    assert_eq!(event.timeout_ms, Some(500));
    assert_eq!(
        (event.board_width, event.board_height),
        (Some(11), Some(11))
    );
    assert_eq!(event.snakes, 4);
    assert_eq!(event.we_survived, None);
}

#[test]
fn an_ended_event_counts_who_is_left_and_whether_we_are_among_them() {
    let alive = LifecycleEvent::ended(&request(3));
    let mut without_us = request(3);
    let us = without_us.you.id.clone();
    without_us.board.snakes.retain(|snake| snake.id != us);
    let dead = LifecycleEvent::ended(&without_us);

    assert_eq!(alive.event, "game_ended");
    assert_eq!((alive.snakes, alive.we_survived), (3, Some(true)));
    assert_eq!((dead.snakes, dead.we_survived), (2, Some(false)));
    assert_eq!(alive.ruleset, None);
    assert_eq!(alive.timeout_ms, None);
}

#[test]
fn the_turn_and_the_game_id_are_made_safe_like_the_decision_events() {
    let mut odd = request(2);
    odd.turn = -5;
    odd.game.id = String::new();

    let event = LifecycleEvent::started(&odd);

    assert_eq!(event.turn, 0);
    assert_eq!(event.game_id, "unknown");
}

#[test]
fn both_events_validate_against_the_lifecycle_contract() {
    for snakes in 0..=4 {
        for event in [
            LifecycleEvent::started(&request(snakes)),
            LifecycleEvent::ended(&request(snakes)),
        ] {
            assert_eq!(
                validate(&contract(), &json(&event)),
                Ok(()),
                "{snakes} snakes: {}",
                json(&event)
            );
        }
    }
}

#[test]
fn an_event_carries_only_what_its_kind_needs_and_never_snake_data() {
    let started = json(&LifecycleEvent::started(&request(4))).to_string();
    let ended = json(&LifecycleEvent::ended(&request(4))).to_string();

    for text in [&started, &ended] {
        for forbidden in ["snake-", "body", "head", "name", "shout", "x\":", "food"] {
            assert!(!text.contains(forbidden), "`{forbidden}` in {text}");
        }
    }
    assert!(
        !started.contains("we_survived")
            && !ended.contains("ruleset")
            && !ended.contains("board_width")
    );
}

#[test]
fn the_contract_check_rejects_a_broken_event() {
    let good = json(&LifecycleEvent::started(&request(2)));
    let mut extra = good.clone();
    extra["snake_names"] = serde_json::json!(["a"]);
    let mut wrong_kind = good.clone();
    wrong_kind["event"] = serde_json::json!("game_paused");
    let mut missing = good.clone();
    missing.as_object_mut().unwrap().remove("game_id");

    assert_eq!(validate(&contract(), &good), Ok(()));
    for broken in [extra, wrong_kind, missing] {
        assert!(validate(&contract(), &broken).is_err());
    }
}

#[test]
fn the_tracing_beacon_emits_exactly_one_line_per_event_with_the_events_json() {
    let started = LifecycleEvent::started(&request(4));
    let ended = LifecycleEvent::ended(&request(2));

    let lines = json_lines(|| {
        TracingLifecycleBeacon.emit(&started);
        TracingLifecycleBeacon.emit(&ended);
    });

    assert_eq!(lines.len(), 2);
    for (line, event) in lines.iter().zip([&started, &ended]) {
        assert_eq!(line["target"], "game_lifecycle");
        assert_eq!(line["level"], "INFO");
        let payload: Value =
            serde_json::from_str(line["fields"]["message"].as_str().unwrap()).unwrap();
        assert_eq!(payload, json(event));
    }
}

proptest! {
    #[test]
    fn any_request_shape_gives_valid_events(snakes in 0usize..=4, turn in any::<i64>(), timeout in any::<i64>()) {
        let mut shaped = request(snakes);
        shaped.turn = turn;
        shaped.game.timeout = timeout;

        for event in [LifecycleEvent::started(&shaped), LifecycleEvent::ended(&shaped)] {
            prop_assert_eq!(validate(&contract(), &json(&event)), Ok(()));
        }
    }
}
