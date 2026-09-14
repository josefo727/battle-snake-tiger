use std::time::{Duration, Instant};

use tiger_sparring::benchmark::Launcher;
use tiger_sparring::launcher::ProcessLauncher;
use tiger_sparring::roster::{Entry, Launch};

fn entry(program: &str, args: &[&str]) -> Entry {
    Entry {
        id: "server".to_owned(),
        launch: Launch {
            program: program.to_owned(),
            args: args.iter().map(|a| (*a).to_owned()).collect(),
            env: Default::default(),
        },
    }
}

#[test]
fn a_program_that_does_not_exist_is_reported_by_name() {
    let error = ProcessLauncher::new(Duration::from_secs(1))
        .start(&entry("/nonexistent/server", &[]))
        .err()
        .expect("cannot start");

    assert!(error.contains("/nonexistent/server"), "{error}");
}

#[test]
fn a_program_that_exits_at_once_is_reported_as_exited_not_waited_for() {
    let started = Instant::now();

    let error = ProcessLauncher::new(Duration::from_secs(5))
        .start(&entry("/bin/sh", &["-c", "exit 3"]))
        .err()
        .expect("exits early");

    assert!(error.contains("exited early"), "{error}");
    assert!(
        started.elapsed() < Duration::from_secs(4),
        "it must not wait out the patience"
    );
}

#[test]
fn a_program_that_never_listens_is_reported_after_the_patience_and_stopped() {
    let started = Instant::now();

    let error = ProcessLauncher::new(Duration::from_millis(400))
        .start(&entry("/bin/sleep", &["30"]))
        .err()
        .expect("never listens");

    assert!(error.contains("never became reachable"), "{error}");
    assert!(started.elapsed() < Duration::from_secs(4));
}
