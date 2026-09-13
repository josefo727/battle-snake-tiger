//! The four Battlesnake webhook routes.

use std::sync::Arc;

use axum::Json;
use axum::Router;
use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, State};
use axum::http::{HeaderMap, StatusCode, header::CONTENT_TYPE};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use serde::Serialize;

use super::beacon::{DecisionBeacon, DecisionEvent};
use crate::ENGINE_VERSION;
use crate::rules_core::{Clock, MoveResponseDto, TurnRequestDto, direction_to_wire};
use crate::verdict::service::VerdictService;

/// The largest request body accepted before any decision work starts.
const MAX_BODY_BYTES: usize = 64 * 1024;

/// The registration metadata served at `GET /`.
#[derive(Debug, Serialize)]
struct Identity {
    apiversion: &'static str,
    author: &'static str,
    color: &'static str,
    head: &'static str,
    tail: &'static str,
    version: &'static str,
}

const IDENTITY: Identity = Identity {
    apiversion: "1",
    author: "josefo727",
    color: "#00D5FF",
    head: "tiger-king",
    tail: "tiger-tail",
    version: ENGINE_VERSION,
};

/// The empty JSON object `/start` and `/end` answer with.
#[derive(Debug, Serialize)]
struct Nothing {}

#[derive(Clone)]
struct Gateway {
    clock: Arc<dyn Clock>,
    service: Arc<VerdictService>,
    beacon: Arc<dyn DecisionBeacon>,
}

/// Builds the router over an already configured service. The clock is the one
/// the service reads; the gateway stamps each request's arrival with it.
pub fn router(
    clock: Arc<dyn Clock>,
    service: VerdictService,
    beacon: Arc<dyn DecisionBeacon>,
) -> Router {
    Router::new()
        .route("/", get(identity))
        .route("/start", post(acknowledge))
        .route("/end", post(acknowledge))
        .route("/move", post(choose_move))
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
        .with_state(Gateway {
            clock,
            service: Arc::new(service),
            beacon,
        })
}

async fn identity() -> Json<Identity> {
    Json(IDENTITY)
}

/// `/start` and `/end` share one acknowledgement: any syntactically valid game
/// gets the empty object, whatever its scope.
async fn acknowledge(Json(_game): Json<TurnRequestDto>) -> Json<Nothing> {
    Json(Nothing {})
}

async fn choose_move(State(gateway): State<Gateway>, headers: HeaderMap, body: Bytes) -> Response {
    if !has_json_content_type(&headers) {
        return StatusCode::UNSUPPORTED_MEDIA_TYPE.into_response();
    }

    let arrived_at = gateway.clock.now();
    let Ok(request) = serde_json::from_slice::<TurnRequestDto>(&body) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let (game_id, turn) = (request.game.id.clone(), request.turn);

    // The search is CPU-bound for up to the whole response window, so it runs off
    // the async workers and never stalls other games' requests.
    let service = Arc::clone(&gateway.service);
    let Ok(report) =
        tokio::task::spawn_blocking(move || service.decide(&request, arrived_at)).await
    else {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };

    gateway
        .beacon
        .emit(&DecisionEvent::new(&report, &game_id, turn));
    Json(MoveResponseDto {
        r#move: direction_to_wire(report.selected_move),
    })
    .into_response()
}

/// Whether `Content-Type` is present and its essence (ignoring parameters such
/// as `charset`) is `application/json`, compared case-insensitively.
fn has_json_content_type(headers: &HeaderMap) -> bool {
    headers
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .is_some_and(|essence| essence.trim().eq_ignore_ascii_case("application/json"))
}
