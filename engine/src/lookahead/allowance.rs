//! How long a search may run: a deadline derived from the response deadline and
//! a cheap, clock-backed stop check.

use core::time::Duration;

use crate::rules_core::{Clock, MonotonicInstant, RequestTiming, response_deadline};

/// Time withheld from the response deadline for serialization and scheduling.
pub const SEARCH_TAIL_MARGIN: Duration = Duration::from_millis(10);

/// Visited nodes between two reads of the clock.
pub const POLL_INTERVAL_NODES: u32 = 1_024;

/// Anything the search consults, once per node, to learn whether it must stop.
pub trait StopSignal {
    /// Counts one visited node and answers whether the search must stop.
    fn should_stop(&mut self) -> bool;
}

/// A signal that never stops, for searches that run to their fixed depth.
#[derive(Clone, Copy, Debug, Default)]
pub struct NeverStop;

impl StopSignal for NeverStop {
    fn should_stop(&mut self) -> bool {
        false
    }
}

/// The stop condition for one search. The clock is read only every
/// [`POLL_INTERVAL_NODES`] visited nodes and on demand, and once the deadline has
/// been seen the answer stays "stop" without reading the clock again.
pub struct SearchAllowance<'clock> {
    clock: &'clock dyn Clock,
    deadline: MonotonicInstant,
    nodes_since_poll: u32,
    expired: bool,
}

impl<'clock> SearchAllowance<'clock> {
    /// An allowance ending [`SEARCH_TAIL_MARGIN`] before `response_deadline`.
    #[must_use]
    pub fn new(clock: &'clock dyn Clock, response_deadline: MonotonicInstant) -> Self {
        let margin = u64::try_from(SEARCH_TAIL_MARGIN.as_micros()).unwrap_or(u64::MAX);
        Self {
            clock,
            deadline: MonotonicInstant {
                microseconds: response_deadline.microseconds.saturating_sub(margin),
            },
            nodes_since_poll: 0,
            expired: false,
        }
    }

    /// An allowance ending exactly at `deadline` (a search deadline that already
    /// took the tail margin), for the lanes of a split search.
    #[must_use]
    pub const fn with_deadline(clock: &'clock dyn Clock, deadline: MonotonicInstant) -> Self {
        Self {
            clock,
            deadline,
            nodes_since_poll: 0,
            expired: false,
        }
    }

    /// The allowance for a request that arrived at `timing.arrived_at` with the
    /// declared `timeout`, or `None` when the timeout does not exceed the
    /// response reserve (such a request is outside the supported scope).
    #[must_use]
    pub fn from_request(
        clock: &'clock dyn Clock,
        timing: RequestTiming,
        declared_timeout: Duration,
    ) -> Option<Self> {
        response_deadline(timing, declared_timeout).map(|deadline| Self::new(clock, deadline))
    }

    #[must_use]
    pub const fn deadline(&self) -> MonotonicInstant {
        self.deadline
    }

    /// Counts one visited node and answers whether the search must stop. The
    /// clock is read only when the node count reaches [`POLL_INTERVAL_NODES`].
    pub fn should_stop(&mut self) -> bool {
        if self.expired {
            return true;
        }
        self.nodes_since_poll += 1;
        if self.nodes_since_poll >= POLL_INTERVAL_NODES {
            return self.poll_now();
        }
        false
    }

    /// Reads the clock now (at an iteration boundary, for example) and answers
    /// whether the deadline has been reached; restarts the node count.
    pub fn poll_now(&mut self) -> bool {
        self.time_left().is_zero()
    }

    /// How much of the allowance remains, read from the clock now; zero once the
    /// deadline has been reached. Restarts the node count like [`Self::poll_now`].
    pub fn time_left(&mut self) -> Duration {
        if self.expired {
            return Duration::ZERO;
        }
        self.nodes_since_poll = 0;
        let now = self.clock.now();
        let left = self.deadline.microseconds.saturating_sub(now.microseconds);
        self.expired = left == 0;
        Duration::from_micros(left)
    }
}

impl StopSignal for SearchAllowance<'_> {
    fn should_stop(&mut self) -> bool {
        Self::should_stop(self)
    }
}
