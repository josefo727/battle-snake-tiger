//! Appending the server's log lines to daily files.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write as _};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};

use tracing_subscriber::fmt::MakeWriter;

use super::calendar::{Calendar, CivilDate};

/// The file that holds the log of `date`: `tiger.log.YYYY-MM-DD`.
#[must_use]
pub fn file_name(date: CivilDate) -> String {
    format!("tiger.log.{date}")
}

struct Shared {
    directory: PathBuf,
    keep_days: usize,
    calendar: Arc<dyn Calendar>,
    open: Mutex<Option<(CivilDate, File)>>,
}

impl Shared {
    /// Appends `bytes` to today's file, opening it first when the day has changed. Every
    /// failure is swallowed: the log is a convenience and must never take the service with it.
    fn append(&self, bytes: &[u8]) {
        let mut open = self.open.lock().unwrap_or_else(PoisonError::into_inner);
        let today = self.calendar.today();
        if open.as_ref().is_none_or(|(date, _)| *date != today) {
            *open = self.open_day(today);
        }
        if let Some((_, file)) = open.as_mut()
            && file.write_all(bytes).is_err()
        {
            // Forget the handle so the next event tries to open the file afresh.
            *open = None;
        }
    }

    /// Deletes the oldest daily files beyond `keep_days` (today's included in the count). Only
    /// regular files named exactly like a daily file are candidates; errors are ignored.
    fn prune(&self) {
        let Ok(entries) = fs::read_dir(&self.directory) else {
            return;
        };
        let mut dated: Vec<(CivilDate, PathBuf)> = entries
            .filter_map(Result::ok)
            .filter(|entry| entry.path().is_file())
            .filter_map(|entry| {
                let date = parse_file_name(entry.file_name().to_str()?)?;
                Some((date, entry.path()))
            })
            .collect();
        dated.sort();
        let surplus = dated.len().saturating_sub(self.keep_days.max(1));
        for (_, path) in dated.into_iter().take(surplus) {
            let _ = fs::remove_file(path);
        }
    }

    fn open_day(&self, date: CivilDate) -> Option<(CivilDate, File)> {
        fs::create_dir_all(&self.directory).ok()?;
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.directory.join(file_name(date)))
            .ok()?;
        self.prune();
        Some((date, file))
    }
}

/// The day a file name stands for, if it is exactly `tiger.log.YYYY-MM-DD`.
#[must_use]
pub fn parse_file_name(name: &str) -> Option<CivilDate> {
    name.strip_prefix("tiger.log.").and_then(CivilDate::parse)
}

/// A `MakeWriter` that appends every event to the daily file of the calendar's
/// current day. Cloning shares the open file.
#[derive(Clone)]
pub struct DailyFileWriter {
    shared: Arc<Shared>,
}

impl DailyFileWriter {
    #[must_use]
    pub fn new(directory: PathBuf, keep_days: usize, calendar: Arc<dyn Calendar>) -> Self {
        Self {
            shared: Arc::new(Shared {
                directory,
                keep_days,
                calendar,
                open: Mutex::new(None),
            }),
        }
    }
}

/// One event's writer: collects the pieces and appends them together, so a line is never
/// split between two files or two writers.
pub struct EventWriter {
    shared: Arc<Shared>,
    buffer: Vec<u8>,
}

impl EventWriter {
    fn hand_over(&mut self) {
        if !self.buffer.is_empty() {
            self.shared.append(&self.buffer);
            self.buffer.clear();
        }
    }
}

impl Drop for EventWriter {
    fn drop(&mut self) {
        self.hand_over();
    }
}

impl io::Write for EventWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.buffer.extend_from_slice(buffer);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.hand_over();
        Ok(())
    }
}

impl<'a> MakeWriter<'a> for DailyFileWriter {
    type Writer = EventWriter;

    fn make_writer(&'a self) -> Self::Writer {
        EventWriter {
            shared: Arc::clone(&self.shared),
            buffer: Vec::new(),
        }
    }
}
