use std::sync::Mutex;

use tiger_engine::gateway::beacon::{DecisionBeacon, DecisionEvent};

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
