use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};

use tiger_engine::gateway::calendar::{Calendar, CivilDate};
use tiger_engine::gateway::logfile::{DailyFileWriter, file_name};
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
