use std::sync::Mutex;

use tiger_engine::gateway::beacon::{DecisionBeacon, DecisionEvent};
use tiger_engine::gateway::lifecycle::{LifecycleBeacon, LifecycleEvent};

/// Keeps every event in memory so a test can count and inspect them.
#[derive(Default)]
pub struct RecordingBeacon {
    events: Mutex<Vec<DecisionEvent>>,
}

impl RecordingBeacon {
    pub fn events(&self) -> Vec<DecisionEvent> {
        self.events
            .lock()
            .expect("the beacon mutex is never poisoned")
            .clone()
    }
}

impl DecisionBeacon for RecordingBeacon {
    fn emit(&self, event: &DecisionEvent) {
        self.events
            .lock()
            .expect("the beacon mutex is never poisoned")
            .push(event.clone());
    }
}

/// Keeps every lifecycle event in memory.
#[derive(Default)]
pub struct RecordingLifecycle {
    events: Mutex<Vec<LifecycleEvent>>,
}

impl RecordingLifecycle {
    pub fn events(&self) -> Vec<LifecycleEvent> {
        self.events
            .lock()
            .expect("the beacon mutex is never poisoned")
            .clone()
    }
}

impl LifecycleBeacon for RecordingLifecycle {
    fn emit(&self, event: &LifecycleEvent) {
        self.events
            .lock()
            .expect("the beacon mutex is never poisoned")
            .push(event.clone());
    }
}
