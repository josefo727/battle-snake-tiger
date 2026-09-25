#![forbid(unsafe_code)]

use std::sync::Arc;

use axum::Router;

use crate::gateway::beacon::DecisionBeacon;
use crate::gateway::lifecycle::LifecycleBeacon;
use crate::lookahead::paranoid::TRADE_RISK;
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
    build_service_with_threads(clock, beacon, lifecycle, 1)
}

/// [`build_service`] with the melee search split over `threads` threads.
pub fn build_service_with_threads(
    clock: Arc<dyn Clock>,
    beacon: Arc<dyn DecisionBeacon>,
    lifecycle: Arc<dyn LifecycleBeacon>,
    threads: usize,
) -> Router {
    build_service_with(clock, beacon, lifecycle, threads, TRADE_RISK)
}

/// The same, with the price a root heading pays for a cell an equal-length
/// rival can also enter. Zero is the sparring opponent that never declines the
/// head-to-head; the local roster had nobody who did.
pub fn build_service_with(
    clock: Arc<dyn Clock>,
    beacon: Arc<dyn DecisionBeacon>,
    lifecycle: Arc<dyn LifecycleBeacon>,
    threads: usize,
    trade_risk: i32,
) -> Router {
    let service = VerdictService::new(clock.clone())
        .with_threads(threads)
        .with_trade_risk(trade_risk);
    gateway::http::router(clock, service, beacon, lifecycle)
}
