use std::time::{Duration, Instant};

use tiger_sparring::benchmark::Launcher;
use tiger_sparring::launcher::ProcessLauncher;
use tiger_sparring::roster::{Entry, Launch, Stop};

fn entry(program: &str, args: &[&str]) -> Entry {
    Entry {
        id: "server".to_owned(),
        launch: Launch {
            program: program.to_owned(),
            args: args.iter().map(|a| (*a).to_owned()).collect(),
            env: Default::default(),
            stop: None,
        },
    }
}

#[test]
fn the_stop_command_runs_when_the_launched_server_is_dropped() {
    // A launch that exits at once is dropped by the launcher itself; the stop
    // command must still run (the container behind a docker client would still
    // be there), with `{port}` filled in like the launch arguments.
    let dir = std::env::temp_dir().join(format!("tiger-launcher-stop-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let marker = dir.join("stopped");
    let mut entry = entry("/bin/sh", &["-c", "exit 0"]);
    entry.launch.stop = Some(Stop {
        program: "sh".to_owned(),
        args: vec![
            "-c".to_owned(),
            format!("echo stopped-{{port}} > '{}'", marker.display()),
        ],
    });

    // Normally the launch fails (exited early) and the launcher drops the guard
    // itself; now and then a concurrent test's connect attempt meets ours in a
    // simultaneous open and the launch "succeeds", in which case dropping the
    // server here must run the stop command all the same.
    let started = ProcessLauncher::new(Duration::from_secs(5)).start(&entry);
    if let Err(error) = &started {
        assert!(error.contains("exited early"), "{error}");
    }
    drop(started);

    let written = std::fs::read_to_string(&marker).expect("the stop command ran");
    let port: u16 = written
        .trim()
        .strip_prefix("stopped-")
        .and_then(|p| p.parse().ok())
        .expect("the port was filled in");
    assert!(port > 0);
    let _ = std::fs::remove_dir_all(&dir);
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
