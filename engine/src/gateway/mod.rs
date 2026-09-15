//! The edge of the service: what enters (webhook routes), what leaves
//! (diagnostics), and the process around them (clock, settings).

pub mod beacon;
pub mod calendar;
pub mod clock;
pub mod http;
pub mod settings;
