use std::thread::sleep;
use std::time::Duration;

use tiger_engine::gateway::clock::SystemClock;
use tiger_engine::rules_core::Clock;

#[test]
fn the_clock_starts_near_zero() {
    let clock = SystemClock::new();

    assert!(clock.now().microseconds < 1_000_000);
}

#[test]
fn consecutive_readings_never_decrease() {
    let clock = SystemClock::new();
    let mut previous = clock.now();

    for _ in 0..10_000 {
        let reading = clock.now();
        assert!(reading >= previous);
        previous = reading;
    }
}

#[test]
fn time_passing_moves_the_clock_by_at_least_that_much() {
    let clock = SystemClock::new();
    let before = clock.now();

    sleep(Duration::from_millis(20));
    let after = clock.now();

    assert!(
        after.microseconds - before.microseconds >= 20_000,
        "{before:?} -> {after:?}"
    );
}

#[test]
fn a_shared_clock_can_be_read_from_several_threads() {
    let clock = std::sync::Arc::new(SystemClock::new());

    let readings: Vec<u64> = (0..4)
        .map(|_| {
            let clock = clock.clone();
            std::thread::spawn(move || clock.now().microseconds)
        })
        .map(|handle| handle.join().unwrap())
        .collect();

    assert_eq!(readings.len(), 4);
}
