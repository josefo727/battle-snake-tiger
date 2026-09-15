#![forbid(unsafe_code)]

use std::sync::Arc;

use axum::Router;

use crate::gateway::beacon::DecisionBeacon;
use crate::gateway::lifecycle::LifecycleBeacon;
use crate::rules_core::Clock;
use crate::verdict::service::VerdictService;

pub mod arena;
pub mod gateway;
pub mod lookahead;
pub mod rules_core;
pub mod valuation;
pub mod verdict;

/// Public engine version advertised by the Battlesnake metadata endpoint.
pub const ENGINE_VERSION: &str = "0.1.0";

/// The composition root: the full router over the production search settings.
pub fn build_service(
    clock: Arc<dyn Clock>,
    beacon: Arc<dyn DecisionBeacon>,
    lifecycle: Arc<dyn LifecycleBeacon>,
) -> Router {
    let service = VerdictService::new(clock.clone());
    gateway::http::router(clock, service, beacon, lifecycle)
}
