use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

use tiger_engine::rules_core::{Clock, MonotonicInstant};

/// A clock that moves only when a test says so and counts how often it is read,
/// so tests can assert both what time it is and how often anyone asked.
pub struct ManualClock {
    microseconds: AtomicU64,
    reads: AtomicUsize,
}

impl ManualClock {
    pub fn at_micros(microseconds: u64) -> Self {
        Self {
            microseconds: AtomicU64::new(microseconds),
            reads: AtomicUsize::new(0),
        }
    }

    pub fn set_micros(&self, microseconds: u64) {
        self.microseconds.store(microseconds, Ordering::SeqCst);
    }

    pub fn reads(&self) -> usize {
        self.reads.load(Ordering::SeqCst)
    }
}

impl Clock for ManualClock {
    fn now(&self) -> MonotonicInstant {
        self.reads.fetch_add(1, Ordering::SeqCst);
        MonotonicInstant {
            microseconds: self.microseconds.load(Ordering::SeqCst),
        }
    }
}
