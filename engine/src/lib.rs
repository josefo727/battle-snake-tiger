#![forbid(unsafe_code)]

pub mod arena;
pub mod lookahead;
pub mod rules_core;
pub mod valuation;

/// Public engine version advertised by the Battlesnake metadata endpoint.
pub const ENGINE_VERSION: &str = "0.1.0";
