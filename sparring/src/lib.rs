//! The offline sparring benchmark: statistics over seeded games between the
//! engine and reference opponents. Nothing here is linked into the server.
#![forbid(unsafe_code)]

pub mod ledger;
pub mod runner;
pub mod statistics;
pub mod transcript;
