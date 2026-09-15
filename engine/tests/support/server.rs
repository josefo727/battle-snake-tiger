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
    spawn_in(env, None)
}

/// Like [`spawn`], optionally with a working directory.
pub fn spawn_in(
    env: &[(&str, &str)],
    directory: Option<&std::path::Path>,
) -> (Child, Receiver<String>) {
    let mut command = Command::new(env!("CARGO_BIN_EXE_tiger-engine"));
    if let Some(directory) = directory {
        command.current_dir(directory);
    }
    let mut child = command
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
        Self::start_with(&[], None)
    }

    /// Like [`Server::start`] with extra environment variables and an optional working
    /// directory.
    pub fn start_with(extra: &[(&str, &str)], directory: Option<&std::path::Path>) -> Self {
        let mut env = vec![("BIND_ADDR", "127.0.0.1"), ("PORT", "0")];
        env.extend_from_slice(extra);
        let (child, logs) = spawn_in(&env, directory);
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

/// A third-party server binary started on a free loopback port (it does not
/// announce its address, so the port is chosen first) and killed when dropped.
pub struct ExternalServer {
    child: Child,
    pub addr: SocketAddr,
}

impl ExternalServer {
    /// Starts `binary` with `BIND_ADDR`/`PORT` and waits until it accepts a
    /// connection.
    pub fn start(binary: &std::path::Path) -> Self {
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .and_then(|listener| listener.local_addr())
            .expect("a free port")
            .port();
        let child = Command::new(binary)
            .env_clear()
            .env("BIND_ADDR", "127.0.0.1")
            .env("PORT", port.to_string())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap_or_else(|error| panic!("cannot start {}: {error}", binary.display()));
        let addr = SocketAddr::from(([127, 0, 0, 1], port));
        let deadline = Instant::now() + PATIENCE;
        while std::net::TcpStream::connect_timeout(&addr, Duration::from_millis(200)).is_err() {
            assert!(
                Instant::now() < deadline,
                "{} never accepted a connection",
                binary.display()
            );
            thread::sleep(Duration::from_millis(50));
        }
        Self { child, addr }
    }
}

impl Drop for ExternalServer {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
