//! The compiled server as a subprocess: started on an ephemeral loopback port and
//! killed when dropped, with its JSON log lines available.

use std::io::{BufRead, BufReader};
use std::net::SocketAddr;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::Value;

pub const PATIENCE: Duration = Duration::from_secs(10);

pub struct Server {
    child: Child,
    pub addr: SocketAddr,
    pub logs: Receiver<String>,
}

/// Starts the binary with exactly the given environment and streams its stdout.
pub fn spawn(env: &[(&str, &str)]) -> (Child, Receiver<String>) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_tiger-engine"))
        .env_clear()
        .envs(env.iter().copied())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("the binary starts");
    let stdout = child.stdout.take().expect("stdout is piped");
    let (sender, logs) = mpsc::channel();
    thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if sender.send(line).is_err() {
                break;
            }
        }
    });
    (child, logs)
}

impl Server {
    /// Starts the binary on an ephemeral loopback port and waits for it to say
    /// where it listens.
    pub fn start() -> Self {
        let (child, logs) = spawn(&[("BIND_ADDR", "127.0.0.1"), ("PORT", "0")]);
        let mut server = Self {
            child,
            addr: SocketAddr::from(([127, 0, 0, 1], 0)),
            logs,
        };
        let announced = server.wait_for_log(|line| line["fields"]["message"] == "listening");
        server.addr = announced["fields"]["addr"]
            .as_str()
            .expect("the announcement carries the address")
            .parse()
            .expect("a socket address");
        server
    }

    /// The next log line, as JSON, that satisfies `wanted`.
    pub fn wait_for_log(&self, wanted: impl Fn(&Value) -> bool) -> Value {
        let deadline = Instant::now() + PATIENCE;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            let line = self
                .logs
                .recv_timeout(left)
                .expect("the server logged the expected line in time");
            if let Ok(json) = serde_json::from_str::<Value>(&line)
                && wanted(&json)
            {
                return json;
            }
        }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
