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

/// A clock that answers each read from a script (microseconds, one entry per
/// read, the last entry repeating), so a test controls exactly what the search
/// sees at every boundary.
pub struct ScriptedClock {
    script: Vec<u64>,
    reads: AtomicUsize,
}

impl ScriptedClock {
    pub fn new(script: &[u64]) -> Self {
        assert!(!script.is_empty(), "a script needs at least one reading");
        Self {
            script: script.to_vec(),
            reads: AtomicUsize::new(0),
        }
    }

    /// Reads at `0` for the first `reads_before_expiry` reads, then far past any
    /// deadline this suite uses.
    pub fn expiring_after(reads_before_expiry: usize) -> Self {
        let mut script = vec![0; reads_before_expiry];
        script.push(u64::MAX / 2);
        Self::new(&script)
    }

    pub fn reads(&self) -> usize {
        self.reads.load(Ordering::SeqCst)
    }
}

impl Clock for ScriptedClock {
    fn now(&self) -> MonotonicInstant {
        let index = self.reads.fetch_add(1, Ordering::SeqCst);
        MonotonicInstant {
            microseconds: self.script[index.min(self.script.len() - 1)],
        }
    }
}
