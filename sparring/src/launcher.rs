//! Starting real server processes on free loopback ports.

use std::net::{SocketAddr, TcpListener, TcpStream};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use crate::benchmark::{Launcher, RunningServer};
use crate::roster::Entry;

/// Stops its process when dropped, so no server outlives a benchmark.
struct ChildGuard(Child);

impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Launches each roster entry as a child process with its output discarded.
pub struct ProcessLauncher {
    patience: Duration,
}

impl ProcessLauncher {
    /// A launcher that waits up to `patience` for a server to accept connections.
    #[must_use]
    pub const fn new(patience: Duration) -> Self {
        Self { patience }
    }
}

impl Launcher for ProcessLauncher {
    fn start(&self, entry: &Entry) -> Result<RunningServer, String> {
        // The port is chosen first because the opponents do not announce theirs; the
        // gap before the server binds it is the usual small race.
        let port = TcpListener::bind("127.0.0.1:0")
            .and_then(|listener| listener.local_addr())
            .map_err(|error| format!("no free port: {error}"))?
            .port();
        let command = entry.launch.command(port);
        let child = Command::new(&command.program)
            .args(&command.args)
            .envs(&command.env)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|error| format!("cannot start {}: {error}", command.program))?;
        let mut guard = ChildGuard(child);

        let address = SocketAddr::from(([127, 0, 0, 1], port));
        let deadline = Instant::now() + self.patience;
        loop {
            if TcpStream::connect_timeout(&address, Duration::from_millis(200))
                .is_ok_and(|stream| !is_self_connection(&stream))
            {
                return Ok(RunningServer::new(
                    format!("http://{address}"),
                    Box::new(guard),
                ));
            }
            if let Ok(Some(status)) = guard.0.try_wait() {
                return Err(format!("{} exited early with {status}", command.program));
            }
            if Instant::now() >= deadline {
                return Err(format!(
                    "{} never became reachable on port {port} within {:?}",
                    command.program, self.patience
                ));
            }
            thread::sleep(Duration::from_millis(50));
        }
    }
}

/// Connecting to a closed port in the ephemeral range can, now and then, connect the socket to
/// itself (its own source port equals the destination port); that is not a server answering.
fn is_self_connection(stream: &TcpStream) -> bool {
    stream.local_addr().ok() == stream.peer_addr().ok()
}
