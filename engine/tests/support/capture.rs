use std::io;
use std::sync::{Arc, Mutex};

use serde_json::Value;
use tracing_subscriber::fmt::MakeWriter;

/// A tracing writer that keeps everything written to it.
#[derive(Clone, Default)]
pub struct Capture(Arc<Mutex<Vec<u8>>>);

impl io::Write for Capture {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl<'a> MakeWriter<'a> for Capture {
    type Writer = Self;

    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

/// Runs `emit` under a JSON tracing subscriber and returns every log line it produced, parsed.
pub fn json_lines(emit: impl FnOnce()) -> Vec<Value> {
    let capture = Capture::default();
    let subscriber = tracing_subscriber::fmt()
        .json()
        .with_writer(capture.clone())
        .with_max_level(tracing::Level::INFO)
        .finish();
    tracing::subscriber::with_default(subscriber, emit);
    let text = String::from_utf8(capture.0.lock().unwrap().clone()).unwrap();
    text.lines()
        .map(|line| serde_json::from_str(line).expect("a JSON log line"))
        .collect()
}
