mod support;

use std::sync::Arc;

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use proptest::prelude::*;
use serde_json::Value;
use tiger_engine::build_service;
use tiger_engine::gateway::beacon::DecisionEvent;
use tiger_engine::gateway::http::router;
use tiger_engine::rules_core::{Clock, MonotonicInstant};
use tiger_engine::verdict::service::VerdictService;
use tower::ServiceExt;

use support::beacon::{RecordingBeacon, RecordingLifecycle};
use support::clock::{FailingClock, ManualClock};
use support::request_json_with;

const IDENTITY: &str = r##"{"apiversion":"1","author":"josefo727","color":"#00D5FF","head":"tiger-king","tail":"tiger-tail","version":"0.1.0"}"##;
const ARRIVAL: u64 = 1_000_000;

struct Harness {
    app: Router,
    beacon: Arc<RecordingBeacon>,
    lifecycle: Arc<RecordingLifecycle>,
}

/// The real router over a service capped at two plies, on a clock that stands still.
fn harness() -> Harness {
    let clock = Arc::new(ManualClock::at_micros(ARRIVAL));
    let beacon = Arc::new(RecordingBeacon::default());
    let service = VerdictService::new(clock.clone()).with_depth_limit(2);
    let lifecycle = Arc::new(RecordingLifecycle::default());
    Harness {
        app: router(clock, service, beacon.clone(), lifecycle.clone()),
        beacon,
        lifecycle,
    }
}

fn json_post(path: &str, body: &str) -> Request<Body> {
    raw_post(path, Some("application/json"), body.as_bytes().to_vec())
}

fn raw_post(path: &str, content_type: Option<&str>, body: Vec<u8>) -> Request<Body> {
    let mut builder = Request::builder().method("POST").uri(path);
    if let Some(content_type) = content_type {
        builder = builder.header(header::CONTENT_TYPE, content_type);
    }
    builder
        .body(Body::from(body))
        .expect("a valid test request")
}

struct Answer {
    status: StatusCode,
    content_type: Option<String>,
    body: Vec<u8>,
}

async fn send(app: &Router, request: Request<Body>) -> Answer {
    let response = app
        .clone()
        .oneshot(request)
        .await
        .expect("the router answers");
    let status = response.status();
    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .map(|value| value.to_str().unwrap().to_owned());
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap()
        .to_vec();
    Answer {
        status,
        content_type,
        body,
    }
}

fn duel_json() -> String {
    request_json_with("v1.2.3", 2, 500, 0, &[(8, 8)])
}

// ---- GET / -------------------------------------------------------------------

#[tokio::test]
async fn get_root_returns_exactly_the_registered_identity() {
    let h = harness();

    let answer = send(
        &h.app,
        Request::builder().uri("/").body(Body::empty()).unwrap(),
    )
    .await;

    assert_eq!(answer.status, StatusCode::OK);
    assert_eq!(answer.content_type.as_deref(), Some("application/json"));
    assert_eq!(String::from_utf8(answer.body).unwrap(), IDENTITY);
}

#[tokio::test]
async fn the_composition_root_serves_the_same_identity() {
    let clock: Arc<dyn Clock> = Arc::new(ManualClock::at_micros(ARRIVAL));
    let app = build_service(
        clock,
        Arc::new(RecordingBeacon::default()),
        Arc::new(RecordingLifecycle::default()),
    );

    let answer = send(
        &app,
        Request::builder().uri("/").body(Body::empty()).unwrap(),
    )
    .await;

    assert_eq!(String::from_utf8(answer.body).unwrap(), IDENTITY);
}

// ---- POST /start and /end -------------------------------------------------------

#[tokio::test]
async fn start_and_end_acknowledge_any_syntactically_valid_game_with_an_empty_object() {
    let h = harness();
    let bodies = [
        duel_json(),
        request_json_with("v1.2.3", 3, 500, 0, &[]),
        request_json_with("v9.9.9", 2, 500, 0, &[]),
    ];

    for path in ["/start", "/end"] {
        for body in &bodies {
            let answer = send(&h.app, json_post(path, body)).await;

            assert_eq!(answer.status, StatusCode::OK, "{path}");
            assert_eq!(answer.content_type.as_deref(), Some("application/json"));
            assert_eq!(answer.body, b"{}", "{path}");
        }
    }
    assert!(
        h.beacon.events().is_empty(),
        "lifecycle events are not decisions"
    );
}

#[tokio::test]
async fn start_and_end_reject_unusable_bodies_like_move_does() {
    let h = harness();

    for path in ["/start", "/end"] {
        let wrong_type = send(
            &h.app,
            raw_post(path, Some("text/plain"), duel_json().into_bytes()),
        )
        .await;
        let malformed = send(&h.app, json_post(path, "{not json")).await;
        let oversized = send(
            &h.app,
            raw_post(path, Some("application/json"), vec![b' '; 70_000]),
        )
        .await;
        let incomplete = send(&h.app, json_post(path, r#"{"game":{}}"#)).await;

        assert_eq!(
            wrong_type.status,
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "{path}"
        );
        assert_eq!(malformed.status, StatusCode::BAD_REQUEST, "{path}");
        assert_eq!(oversized.status, StatusCode::PAYLOAD_TOO_LARGE, "{path}");
        assert!(
            incomplete.status.is_client_error(),
            "{path}: {}",
            incomplete.status
        );
    }
}

// ---- POST /move -----------------------------------------------------------------

fn only_move(answer: &Answer) -> String {
    let json: Value = serde_json::from_slice(&answer.body).expect("a JSON body");
    let members = json.as_object().expect("an object");
    assert_eq!(members.len(), 1, "exactly one member: {json}");
    let direction = members["move"].as_str().expect("a string move").to_owned();
    assert!(
        ["up", "right", "down", "left"].contains(&direction.as_str()),
        "{direction}"
    );
    direction
}

#[tokio::test]
async fn a_duel_a_melee_and_an_unsupported_game_each_get_a_move_and_one_diagnostic() {
    let cases = [
        ("duel", duel_json(), "duel_search"),
        (
            "melee",
            request_json_with("v1.2.3", 3, 500, 0, &[]),
            "melee_search",
        ),
        (
            "four snakes",
            request_json_with("v1.2.3", 4, 500, 2, &[(5, 6)]),
            "melee_search",
        ),
        (
            "unsupported",
            request_json_with("v9.9.9", 2, 500, 0, &[]),
            "unsupported_fallback",
        ),
    ];

    for (label, body, path) in cases {
        let h = harness();

        let answer = send(&h.app, json_post("/move", &body)).await;

        assert_eq!(answer.status, StatusCode::OK, "{label}");
        assert_eq!(
            answer.content_type.as_deref(),
            Some("application/json"),
            "{label}"
        );
        let chosen = only_move(&answer);
        let events = h.beacon.events();
        assert_eq!(events.len(), 1, "{label}: one diagnostic per decision");
        assert_eq!(events[0].engine_path, path, "{label}");
        assert_eq!(
            events[0].selected_move, chosen,
            "{label}: the event names the served move"
        );
        assert_eq!(events[0].game_id, "test-game", "{label}");
    }
}

#[tokio::test]
async fn the_duel_move_comes_from_the_search_at_the_configured_depth() {
    let h = harness();

    let answer = send(&h.app, json_post("/move", &duel_json())).await;

    let events: Vec<DecisionEvent> = h.beacon.events();
    assert_eq!(events[0].search_depth, 2);
    assert!(events[0].nodes_explored > 0);
    assert_eq!(only_move(&answer), events[0].selected_move);
}

#[tokio::test]
async fn the_served_move_equals_the_services_own_decision_for_the_same_request() {
    let h = harness();
    let clock = Arc::new(ManualClock::at_micros(ARRIVAL));
    let direct = VerdictService::new(clock).with_depth_limit(2).decide(
        &serde_json::from_str(&duel_json()).unwrap(),
        MonotonicInstant {
            microseconds: ARRIVAL,
        },
    );

    let answer = send(&h.app, json_post("/move", &duel_json())).await;

    assert_eq!(
        only_move(&answer),
        tiger_engine::rules_core::direction_to_wire(direct.selected_move)
    );
}

#[tokio::test]
async fn move_rejects_the_wrong_or_missing_content_type_without_deciding() {
    let h = harness();

    let wrong = send(
        &h.app,
        raw_post("/move", Some("text/plain"), duel_json().into_bytes()),
    )
    .await;
    let missing = send(&h.app, raw_post("/move", None, duel_json().into_bytes())).await;

    assert_eq!(wrong.status, StatusCode::UNSUPPORTED_MEDIA_TYPE);
    assert_eq!(missing.status, StatusCode::UNSUPPORTED_MEDIA_TYPE);
    assert!(h.beacon.events().is_empty());
}

#[tokio::test]
async fn move_accepts_json_with_parameters_and_any_case() {
    let h = harness();

    for content_type in ["application/json; charset=utf-8", "Application/JSON"] {
        let answer = send(
            &h.app,
            raw_post("/move", Some(content_type), duel_json().into_bytes()),
        )
        .await;

        assert_eq!(answer.status, StatusCode::OK, "{content_type}");
    }
}

#[tokio::test]
async fn move_rejects_malformed_json_without_echoing_it_or_deciding() {
    let h = harness();

    let answer = send(&h.app, json_post("/move", "{\"secret\": [1, 2,")).await;

    assert_eq!(answer.status, StatusCode::BAD_REQUEST);
    assert!(!String::from_utf8_lossy(&answer.body).contains("secret"));
    assert!(h.beacon.events().is_empty());
}

#[tokio::test]
async fn move_rejects_a_body_over_64_kib_without_deciding() {
    let h = harness();

    let answer = send(
        &h.app,
        raw_post("/move", Some("application/json"), vec![b' '; 65 * 1024]),
    )
    .await;

    assert_eq!(answer.status, StatusCode::PAYLOAD_TOO_LARGE);
    assert!(h.beacon.events().is_empty());
}

#[tokio::test]
async fn a_body_just_under_the_limit_still_reaches_the_decision() {
    let h = harness();
    let mut body = duel_json();
    body.push_str(&" ".repeat(64 * 1024 - body.len() - 1));

    let answer = send(&h.app, json_post("/move", &body)).await;

    assert_eq!(answer.status, StatusCode::OK);
    assert_eq!(h.beacon.events().len(), 1);
}

#[tokio::test]
async fn a_panic_inside_the_decision_is_a_500_and_leaves_the_router_serving() {
    // One healthy read (the arrival stamp), then the clock fails inside the search.
    let clock = Arc::new(FailingClock::after(1));
    let beacon = Arc::new(RecordingBeacon::default());
    let service = VerdictService::new(clock.clone()).with_depth_limit(2);
    let app = router(
        clock,
        service,
        beacon.clone(),
        Arc::new(RecordingLifecycle::default()),
    );

    let failed = send(&app, json_post("/move", &duel_json())).await;
    let identity = send(
        &app,
        Request::builder().uri("/").body(Body::empty()).unwrap(),
    )
    .await;

    assert_eq!(failed.status, StatusCode::INTERNAL_SERVER_ERROR);
    assert!(
        beacon.events().is_empty(),
        "a failed decision emits no diagnostic"
    );
    assert_eq!(String::from_utf8(identity.body).unwrap(), IDENTITY);
}

#[tokio::test]
async fn unknown_paths_are_404_and_wrong_methods_405() {
    let h = harness();

    let missing = send(
        &h.app,
        Request::builder().uri("/nope").body(Body::empty()).unwrap(),
    )
    .await;
    let wrong_method = send(
        &h.app,
        Request::builder().uri("/move").body(Body::empty()).unwrap(),
    )
    .await;

    assert_eq!(missing.status, StatusCode::NOT_FOUND);
    assert_eq!(wrong_method.status, StatusCode::METHOD_NOT_ALLOWED);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(40))]

    #[test]
    fn every_syntactically_valid_move_request_gets_a_200_with_one_platform_move(
        version in prop_oneof![Just("v1.2.3"), Just("cli"), Just("v9.9.9")],
        snakes in 1usize..=4,
        timeout in 100i64..=600,
    ) {
        let runtime = tokio::runtime::Builder::new_current_thread().build().unwrap();
        let h = harness();
        let body = request_json_with(version, snakes, timeout, 0, &[]);

        let answer = runtime.block_on(send(&h.app, json_post("/move", &body)));

        prop_assert_eq!(answer.status, StatusCode::OK);
        let _ = only_move(&answer);
        prop_assert_eq!(h.beacon.events().len(), 1);
    }
}

// ---- lifecycle events -------------------------------------------------------------------------

#[tokio::test]
async fn a_valid_start_and_a_valid_end_each_emit_one_lifecycle_event_and_the_same_empty_answer() {
    let h = harness();
    let body = request_json_with("v1.2.3", 4, 500, 0, &[]);

    let started = send(&h.app, json_post("/start", &body)).await;
    let ended = send(&h.app, json_post("/end", &body)).await;

    assert_eq!(
        (started.status, started.body.as_slice()),
        (StatusCode::OK, &b"{}"[..])
    );
    assert_eq!(
        (ended.status, ended.body.as_slice()),
        (StatusCode::OK, &b"{}"[..])
    );
    let events = h.lifecycle.events();
    assert_eq!(events.len(), 2);
    assert_eq!(
        (
            events[0].event,
            events[0].game_id.as_str(),
            events[0].snakes
        ),
        ("game_started", "test-game", 4)
    );
    assert_eq!(events[0].ruleset.as_deref(), Some("standard/v1.2.3"));
    assert_eq!(
        (events[1].event, events[1].we_survived),
        ("game_ended", Some(true))
    );
}

#[tokio::test]
async fn a_game_outside_the_certified_scope_still_reports_its_start() {
    let h = harness();

    send(
        &h.app,
        json_post("/start", &request_json_with("v9.9.9", 2, 500, 0, &[])),
    )
    .await;

    let events = h.lifecycle.events();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].ruleset.as_deref(), Some("standard/v9.9.9"));
}

#[tokio::test]
async fn a_rejected_request_and_a_move_emit_no_lifecycle_event() {
    let h = harness();

    send(&h.app, json_post("/start", "{not json")).await;
    send(
        &h.app,
        raw_post("/end", Some("text/plain"), duel_json().into_bytes()),
    )
    .await;
    send(
        &h.app,
        raw_post("/start", Some("application/json"), vec![b' '; 70_000]),
    )
    .await;
    send(&h.app, json_post("/end", r#"{"game":{}}"#)).await;
    send(&h.app, json_post("/move", &duel_json())).await;

    assert!(
        h.lifecycle.events().is_empty(),
        "{:?}",
        h.lifecycle.events()
    );
}
