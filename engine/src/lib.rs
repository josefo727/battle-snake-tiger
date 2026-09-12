#![forbid(unsafe_code)]

pub mod arena;
pub mod gateway;
pub mod lookahead;
pub mod rules_core;
pub mod valuation;
pub mod verdict;

/// Public engine version advertised by the Battlesnake metadata endpoint.
pub const ENGINE_VERSION: &str = "0.1.0";
