use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};

use tiger_engine::gateway::calendar::{Calendar, CivilDate};
use tiger_engine::gateway::logfile::{DailyFileWriter, file_name, parse_file_name};
use tracing_subscriber::fmt::MakeWriter;

/// A calendar the test moves by hand, in Unix seconds.
struct FakeCalendar(AtomicI64);

impl FakeCalendar {
    fn at(date: CivilDate) -> Arc<Self> {
        // 12:00 UTC of that day: safely inside it.
        let days = (0..)
            .map(|d| d * 86_400)
            .find(|&s| CivilDate::from_unix_seconds(s) == date)
            .expect("a date after 1970");
        Arc::new(Self(AtomicI64::new(days + 43_200)))
    }

    fn advance_days(&self, days: i64) {
        self.0.fetch_add(days * 86_400, Ordering::SeqCst);
    }
}

impl Calendar for FakeCalendar {
    fn today(&self) -> CivilDate {
        CivilDate::from_unix_seconds(self.0.load(Ordering::SeqCst))
    }
}

fn day(year: i32, month: u8, day: u8) -> CivilDate {
    CivilDate { year, month, day }
}

struct Scratch(PathBuf);

impl Scratch {
    fn new(label: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("tiger-logfile-{}-{label}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn log_one(writer: &DailyFileWriter, line: &str) {
    let mut event = writer.make_writer();
    event
        .write_all(line.as_bytes())
        .expect("writing never fails");
    event.write_all(b"\n").unwrap();
}

fn read(dir: &Path, date: CivilDate) -> String {
    fs::read_to_string(dir.join(file_name(date))).unwrap_or_default()
}

#[test]
fn a_line_lands_in_the_file_of_the_calendars_day() {
    let scratch = Scratch::new("one");
    let calendar = FakeCalendar::at(day(2026, 9, 19));
    let writer = DailyFileWriter::new(scratch.0.clone(), 14, calendar);

    log_one(&writer, r#"{"event":"one"}"#);

    assert_eq!(file_name(day(2026, 9, 19)), "tiger.log.2026-09-19");
    assert_eq!(read(&scratch.0, day(2026, 9, 19)), "{\"event\":\"one\"}\n");
}

#[test]
fn lines_of_one_day_append_in_order() {
    let scratch = Scratch::new("append");
    let writer = DailyFileWriter::new(scratch.0.clone(), 14, FakeCalendar::at(day(2026, 9, 19)));

    log_one(&writer, "first");
    log_one(&writer, "second");
    log_one(&writer, "third");

    assert_eq!(read(&scratch.0, day(2026, 9, 19)), "first\nsecond\nthird\n");
}

#[test]
fn a_new_utc_day_starts_a_new_file_and_leaves_the_old_one_alone() {
    let scratch = Scratch::new("rotate");
    let calendar = FakeCalendar::at(day(2026, 9, 19));
    let writer = DailyFileWriter::new(scratch.0.clone(), 14, calendar.clone());

    log_one(&writer, "before midnight");
    calendar.advance_days(1);
    log_one(&writer, "after midnight");

    assert_eq!(read(&scratch.0, day(2026, 9, 19)), "before midnight\n");
    assert_eq!(read(&scratch.0, day(2026, 9, 20)), "after midnight\n");
}

#[test]
fn a_restart_appends_to_the_days_existing_file() {
    let scratch = Scratch::new("restart");
    let calendar = FakeCalendar::at(day(2026, 9, 19));
    log_one(
        &DailyFileWriter::new(scratch.0.clone(), 14, calendar.clone()),
        "before the restart",
    );

    log_one(
        &DailyFileWriter::new(scratch.0.clone(), 14, calendar),
        "after the restart",
    );

    assert_eq!(
        read(&scratch.0, day(2026, 9, 19)),
        "before the restart\nafter the restart\n"
    );
}

#[test]
fn a_missing_directory_is_created_including_its_parents() {
    let scratch = Scratch::new("mkdir");
    let nested = scratch.0.join("var/log/tiger");
    let writer = DailyFileWriter::new(nested.clone(), 14, FakeCalendar::at(day(2026, 9, 19)));

    log_one(&writer, "hello");

    assert_eq!(read(&nested, day(2026, 9, 19)), "hello\n");
}

#[test]
fn an_event_written_in_pieces_is_never_split_across_two_days() {
    let scratch = Scratch::new("pieces");
    let calendar = FakeCalendar::at(day(2026, 9, 19));
    let writer = DailyFileWriter::new(scratch.0.clone(), 14, calendar.clone());

    let mut event = writer.make_writer();
    event.write_all(b"{\"a\":").unwrap();
    calendar.advance_days(1);
    event.write_all(b"1}\n").unwrap();
    drop(event);

    let both = format!(
        "{}{}",
        read(&scratch.0, day(2026, 9, 19)),
        read(&scratch.0, day(2026, 9, 20))
    );
    assert_eq!(both, "{\"a\":1}\n", "one whole line in exactly one file");
    assert!(
        read(&scratch.0, day(2026, 9, 19)).is_empty()
            != read(&scratch.0, day(2026, 9, 20)).is_empty()
    );
}

#[test]
fn a_write_that_cannot_happen_is_swallowed_and_retried_next_time() {
    let scratch = Scratch::new("blocked");
    // The log directory's path is a regular file, so it can neither be created nor opened.
    let blocked = scratch.0.join("logs");
    fs::write(&blocked, "in the way").unwrap();
    let writer = DailyFileWriter::new(blocked.clone(), 14, FakeCalendar::at(day(2026, 9, 19)));

    log_one(&writer, "lost while blocked");

    fs::remove_file(&blocked).unwrap();
    log_one(&writer, "kept once the way is clear");
    assert_eq!(
        read(&blocked, day(2026, 9, 19)),
        "kept once the way is clear\n"
    );
}

#[test]
fn clones_share_one_file_and_threads_never_interleave_a_line() {
    let scratch = Scratch::new("threads");
    let writer = DailyFileWriter::new(scratch.0.clone(), 14, FakeCalendar::at(day(2026, 9, 19)));

    std::thread::scope(|scope| {
        for thread in 0..8 {
            let writer = writer.clone();
            scope.spawn(move || {
                for n in 0..50 {
                    log_one(
                        &writer,
                        &format!(
                            "{{\"thread\":{thread},\"n\":{n},\"pad\":\"{}\"}}",
                            "x".repeat(200)
                        ),
                    );
                }
            });
        }
    });

    let content = read(&scratch.0, day(2026, 9, 19));
    let lines: Vec<&str> = content.lines().collect();
    assert_eq!(lines.len(), 400);
    for line in lines {
        let value: serde_json::Value = serde_json::from_str(line).expect("a complete JSON line");
        assert!(value["thread"].is_number());
    }
}

#[test]
fn it_works_as_the_writer_of_a_tracing_subscriber() {
    let scratch = Scratch::new("tracing");
    let writer = DailyFileWriter::new(scratch.0.clone(), 14, FakeCalendar::at(day(2026, 9, 19)));
    let subscriber = tracing_subscriber::fmt()
        .json()
        .with_writer(writer)
        .finish();

    tracing::subscriber::with_default(subscriber, || {
        tracing::info!(target: "move_decision", "an event for the file");
    });

    let content = read(&scratch.0, day(2026, 9, 19));
    let line: serde_json::Value = serde_json::from_str(content.trim()).expect("one JSON line");
    assert_eq!(line["target"], "move_decision");
    assert_eq!(line["fields"]["message"], "an event for the file");
}

// ---- retention ----------------------------------------------------------------------------

fn touch(dir: &Path, name: &str) {
    fs::write(dir.join(name), "old").unwrap();
}

fn daily_files(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap())
        .filter(|e| e.path().is_file())
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| parse_file_name(n).is_some())
        .collect();
    names.sort();
    names
}

#[test]
fn only_exact_daily_names_are_daily_files() {
    assert_eq!(
        parse_file_name("tiger.log.2026-09-19"),
        Some(day(2026, 9, 19))
    );
    for other in [
        "tiger.log",
        "tiger.log.",
        "tiger.log.2026-09-19.gz",
        "tiger.log.2026-13-01",
        "tiger.log.2026-9-19",
        "other.log.2026-09-19",
        "Tiger.log.2026-09-19",
        " tiger.log.2026-09-19",
        "notes.txt",
        "",
    ] {
        assert_eq!(parse_file_name(other), None, "{other:?}");
    }
}

#[test]
fn opening_a_new_days_file_keeps_only_the_newest_daily_files() {
    let scratch = Scratch::new("retain");
    for d in 1..=20 {
        touch(&scratch.0, &format!("tiger.log.2026-08-{d:02}"));
    }
    let writer = DailyFileWriter::new(scratch.0.clone(), 14, FakeCalendar::at(day(2026, 9, 19)));

    log_one(&writer, "today");

    let kept = daily_files(&scratch.0);
    assert_eq!(kept.len(), 14, "{kept:?}");
    assert_eq!(kept.first().unwrap(), "tiger.log.2026-08-08");
    assert_eq!(kept.last().unwrap(), "tiger.log.2026-09-19");
}

#[test]
fn nothing_else_in_the_directory_is_ever_touched() {
    let scratch = Scratch::new("careful");
    for d in 1..=5 {
        touch(&scratch.0, &format!("tiger.log.2026-08-{d:02}"));
    }
    for other in [
        "notes.txt",
        "tiger.log",
        "tiger.log.2026-08-01.gz",
        "other.log.2026-01-01",
        "tiger.log.2026-13-01",
    ] {
        touch(&scratch.0, other);
    }
    fs::create_dir(scratch.0.join("tiger.log.2020-01-01")).unwrap();
    let writer = DailyFileWriter::new(scratch.0.clone(), 1, FakeCalendar::at(day(2026, 9, 19)));

    log_one(&writer, "today");

    assert_eq!(daily_files(&scratch.0), ["tiger.log.2026-09-19"]);
    for survivor in [
        "notes.txt",
        "tiger.log",
        "tiger.log.2026-08-01.gz",
        "other.log.2026-01-01",
        "tiger.log.2026-13-01",
    ] {
        assert!(
            scratch.0.join(survivor).is_file(),
            "{survivor} must survive"
        );
    }
    assert!(
        scratch.0.join("tiger.log.2020-01-01").is_dir(),
        "a directory is not a log file"
    );
}

#[test]
fn fewer_files_than_the_limit_are_all_kept() {
    let scratch = Scratch::new("few");
    touch(&scratch.0, "tiger.log.2026-09-01");
    touch(&scratch.0, "tiger.log.2026-09-10");
    let writer = DailyFileWriter::new(scratch.0.clone(), 14, FakeCalendar::at(day(2026, 9, 19)));

    log_one(&writer, "today");

    assert_eq!(daily_files(&scratch.0).len(), 3);
}

#[test]
fn a_limit_of_zero_still_keeps_todays_file() {
    let scratch = Scratch::new("zero");
    touch(&scratch.0, "tiger.log.2026-09-18");
    let writer = DailyFileWriter::new(scratch.0.clone(), 0, FakeCalendar::at(day(2026, 9, 19)));

    log_one(&writer, "today");

    assert_eq!(daily_files(&scratch.0), ["tiger.log.2026-09-19"]);
    assert_eq!(read(&scratch.0, day(2026, 9, 19)), "today\n");
}

#[test]
fn pruning_happens_when_a_day_opens_not_on_every_line() {
    let scratch = Scratch::new("once");
    let calendar = FakeCalendar::at(day(2026, 9, 19));
    let writer = DailyFileWriter::new(scratch.0.clone(), 2, calendar.clone());
    log_one(&writer, "first line of the day");
    touch(&scratch.0, "tiger.log.2026-01-01");

    log_one(&writer, "second line of the same day");
    assert!(
        scratch.0.join("tiger.log.2026-01-01").is_file(),
        "no scan on an ordinary write"
    );

    calendar.advance_days(1);
    log_one(&writer, "first line of the next day");
    assert_eq!(
        daily_files(&scratch.0),
        ["tiger.log.2026-09-19", "tiger.log.2026-09-20"]
    );
}
