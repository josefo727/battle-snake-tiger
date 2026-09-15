//! The two lifecycle events of a game: `game_started` and `game_ended`.
//!
//! The shape is `contracts/game-lifecycle.schema.json` (1.0.0). An event names the game and
//! counts snakes; it never carries a name, a body, a position or a shout.

use serde::Serialize;

use crate::rules_core::TurnRequestDto;

pub const SCHEMA_VERSION: &str = "1.0.0";

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct LifecycleEvent {
    pub level: &'static str,
    pub target: &'static str,
    pub event: &'static str,
    pub schema_version: &'static str,
    pub game_id: String,
    pub turn: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ruleset: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub map: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub board_width: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub board_height: Option<u32>,
    pub snakes: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub we_survived: Option<bool>,
}

impl LifecycleEvent {
    /// The event for a valid `POST /start`.
    #[must_use]
    pub fn started(request: &TurnRequestDto) -> Self {
        Self {
            ruleset: Some(format!(
                "{}/{}",
                request.game.ruleset.name, request.game.ruleset.version
            )),
            map: Some(request.game.map.clone()),
            timeout_ms: Some(u64::try_from(request.game.timeout).unwrap_or(0)),
            board_width: Some(u32::try_from(request.board.width).unwrap_or(0)),
            board_height: Some(u32::try_from(request.board.height).unwrap_or(0)),
            ..Self::about("game_started", request)
        }
    }

    /// The event for a valid `POST /end`.
    #[must_use]
    pub fn ended(request: &TurnRequestDto) -> Self {
        let we_survived = request
            .board
            .snakes
            .iter()
            .any(|snake| snake.id == request.you.id);
        Self {
            we_survived: Some(we_survived),
            ..Self::about("game_ended", request)
        }
    }

    /// The identity every event shares: which game, which turn, how many snakes are on the board.
    /// A negative turn (which the platform never sends) becomes 0 and an empty game id
    /// "unknown", so every event satisfies the contract whatever the request held.
    fn about(event: &'static str, request: &TurnRequestDto) -> Self {
        Self {
            level: "INFO",
            target: "game_lifecycle",
            event,
            schema_version: SCHEMA_VERSION,
            game_id: if request.game.id.is_empty() {
                "unknown".to_owned()
            } else {
                request.game.id.clone()
            },
            turn: u64::try_from(request.turn).unwrap_or(0),
            ruleset: None,
            map: None,
            timeout_ms: None,
            board_width: None,
            board_height: None,
            snakes: request.board.snakes.len(),
            we_survived: None,
        }
    }
}

/// Where lifecycle events go. Like the decision beacon it returns nothing: recording an
/// event must never change a response.
pub trait LifecycleBeacon: Send + Sync {
    fn emit(&self, event: &LifecycleEvent);
}

/// The production beacon: one `tracing` event per lifecycle event on target `game_lifecycle`.
#[derive(Clone, Copy, Debug, Default)]
pub struct TracingLifecycleBeacon;

impl LifecycleBeacon for TracingLifecycleBeacon {
    fn emit(&self, event: &LifecycleEvent) {
        let payload = serde_json::to_string(event).unwrap_or_default();
        tracing::info!(target: "game_lifecycle", "{payload}");
    }
}
