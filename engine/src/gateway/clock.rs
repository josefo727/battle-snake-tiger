//! The production clock.

use std::time::Instant;

use crate::rules_core::{Clock, MonotonicInstant};

/// Microseconds elapsed since the clock was created: monotonic, with no wall-clock
/// or serialization meaning, as the reused `Clock` port requires.
#[derive(Debug)]
pub struct SystemClock {
    origin: Instant,
}

impl SystemClock {
    #[must_use]
    pub fn new() -> Self {
        Self {
            origin: Instant::now(),
        }
    }
}

impl Default for SystemClock {
    fn default() -> Self {
        Self::new()
    }
}

impl Clock for SystemClock {
    fn now(&self) -> MonotonicInstant {
        MonotonicInstant {
            microseconds: u64::try_from(self.origin.elapsed().as_micros()).unwrap_or(u64::MAX),
        }
    }
}
