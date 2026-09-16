//! Starts the compiled binary and talks to it over real loopback TCP.

mod support;

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::Value;

use support::request_json_with;
use support::server::{PATIENCE, Server, spawn};
use tiger_engine::gateway::calendar::{Calendar, SystemCalendar};

const IDENTITY: &str = r##"{"apiversion":"1","author":"josefo727","color":"#00D5FF","head":"tiger-king","tail":"tiger-tail","version":"0.1.0"}"##;

struct Reply {
    status: u16,
    headers: Vec<(String, String)>,
    body: String,
}

impl Reply {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }
}

fn http(addr: SocketAddr, request: &str) -> Reply {
    let mut stream = TcpStream::connect_timeout(&addr, PATIENCE).expect("the server accepts");
    stream.set_read_timeout(Some(PATIENCE)).unwrap();
    stream.write_all(request.as_bytes()).unwrap();
    let mut raw = Vec::new();
    stream
        .read_to_end(&mut raw)
        .expect("the server answers and closes");
    let text = String::from_utf8(raw).expect("a UTF-8 response");
    let (head, body) = text.split_once("\r\n\r\n").expect("a complete response");
    let mut lines = head.lines();
    let status = lines
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|code| code.parse().ok())
        .expect("a status line");
    let headers = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(key, value)| (key.trim().to_owned(), value.trim().to_owned()))
        .collect();
    Reply {
        status,
        headers,
        body: body.to_owned(),
    }
}

fn get(addr: SocketAddr, path: &str) -> Reply {
    http(
        addr,
        &format!("GET {path} HTTP/1.1\r\nHost: test\r\nConnection: close\r\n\r\n"),
    )
}

fn post_json(addr: SocketAddr, path: &str, body: &str) -> Reply {
    http(
        addr,
        &format!(
            "POST {path} HTTP/1.1\r\nHost: test\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        ),
    )
}

#[test]
fn the_binary_serves_the_exact_identity_over_loopback_tcp() {
    let server = Server::start();

    let reply = get(server.addr, "/");

    assert_eq!(reply.status, 200);
    assert_eq!(reply.header("content-type"), Some("application/json"));
    assert_eq!(reply.body, IDENTITY);
}

#[test]
fn the_binary_listens_where_the_settings_say() {
    let server = Server::start();

    assert!(server.addr.ip().is_loopback());
    assert_ne!(server.addr.port(), 0, "an ephemeral port was assigned");
}

#[test]
fn every_kind_of_move_request_is_answered_and_logged_by_the_real_process() {
    let server = Server::start();
    let cases = [
        (
            "duel",
            request_json_with("v1.2.3", 2, 500, 0, &[(8, 8)]),
            "duel_search",
        ),
        (
            "melee",
            request_json_with("v1.2.3", 3, 500, 0, &[]),
            "melee_search",
        ),
        (
            "unsupported",
            request_json_with("v9.9.9", 2, 500, 0, &[]),
            "unsupported_fallback",
        ),
    ];

    for (label, body, path) in cases {
        let started = Instant::now();
        let reply = post_json(server.addr, "/move", &body);
        let elapsed = started.elapsed();

        assert_eq!(reply.status, 200, "{label}");
        let answer: Value = serde_json::from_str(&reply.body).expect("a JSON answer");
        let chosen = answer["move"].as_str().expect("a move");
        assert!(
            ["up", "right", "down", "left"].contains(&chosen),
            "{label}: {chosen}"
        );
        assert!(
            elapsed < Duration::from_millis(500),
            "{label}: answered in {elapsed:?}, over the 500 ms the request declared"
        );

        let logged = server.wait_for_log(|line| line["target"] == "move_decision");
        let event: Value =
            serde_json::from_str(logged["fields"]["message"].as_str().unwrap()).unwrap();
        assert_eq!(event["engine_path"], path, "{label}");
        assert_eq!(event["selected_move"], chosen, "{label}");
        if path == "duel_search" {
            assert!(
                event["search_depth"].as_u64().unwrap() >= 1,
                "{label}: {event}"
            );
            assert!(
                event["nodes_explored"].as_u64().unwrap() > 0,
                "{label}: {event}"
            );
        }
    }
}

#[test]
fn start_and_end_are_acknowledged_over_tcp() {
    let server = Server::start();
    let body = request_json_with("v1.2.3", 2, 500, 0, &[]);

    for path in ["/start", "/end"] {
        let reply = post_json(server.addr, path, &body);

        assert_eq!(reply.status, 200, "{path}");
        assert_eq!(reply.body, "{}", "{path}");
    }
}

#[test]
fn the_binary_refuses_an_invalid_port_and_says_which_setting_is_wrong() {
    let (mut child, logs) = spawn(&[("PORT", "not-a-port")]);

    let deadline = Instant::now() + PATIENCE;
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        assert!(
            Instant::now() < deadline,
            "the binary should exit on bad settings"
        );
        thread::sleep(Duration::from_millis(20));
    };

    assert!(!status.success());
    let output: Vec<String> = logs.try_iter().collect();
    assert!(
        output
            .iter()
            .any(|line| line.contains("PORT") && line.contains("not-a-port")),
        "{output:?}"
    );
}

// ---- the daily log file ---------------------------------------------------------------------

struct Scratch(std::path::PathBuf);

impl Scratch {
    fn new(label: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("tiger-process-{}-{label}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn todays_file(dir: &std::path::Path) -> std::path::PathBuf {
    dir.join(format!("tiger.log.{}", SystemCalendar.today()))
}

/// The lines of `path`, retried briefly: the server writes each event before it answers, but
/// the test may read a moment ahead of the file system.
fn wait_for_lines(path: &std::path::Path, wanted: impl Fn(&str) -> bool) -> String {
    let deadline = std::time::Instant::now() + PATIENCE;
    loop {
        let text = std::fs::read_to_string(path).unwrap_or_default();
        if wanted(&text) || std::time::Instant::now() > deadline {
            return text;
        }
        thread::sleep(Duration::from_millis(25));
    }
}

#[test]
fn with_log_dir_set_the_process_writes_its_events_to_todays_file_and_still_prints_them() {
    let scratch = Scratch::new("logdir");
    let dir = scratch.0.join("logs");
    let server = Server::start_with(&[("LOG_DIR", dir.to_str().unwrap())], None);
    let duel = request_json_with("v1.2.3", 2, 500, 0, &[(8, 8)]);

    post_json(server.addr, "/start", &duel);
    post_json(server.addr, "/move", &duel);
    post_json(server.addr, "/end", &duel);

    let text = wait_for_lines(&todays_file(&dir), |t| {
        t.contains("game_ended") && t.contains("move_decision")
    });
    for wanted in ["listening", "game_started", "move_decision", "game_ended"] {
        assert!(
            text.contains(wanted),
            "`{wanted}` missing from the file:\n{text}"
        );
    }
    for line in text.lines() {
        serde_json::from_str::<Value>(line).expect("every line of the file is JSON");
    }
    server.wait_for_log(|line| line["target"] == "move_decision");
    server.wait_for_log(|line| line["target"] == "game_lifecycle");
}

#[test]
fn an_unwritable_log_dir_never_stops_the_server_from_serving_or_printing() {
    let scratch = Scratch::new("blocked");
    let blocker = scratch.0.join("in-the-way");
    std::fs::write(&blocker, "a file, not a directory").unwrap();
    let server = Server::start_with(&[("LOG_DIR", blocker.join("logs").to_str().unwrap())], None);

    let reply = get(server.addr, "/");
    post_json(
        server.addr,
        "/start",
        &request_json_with("v1.2.3", 2, 500, 0, &[]),
    );

    assert_eq!(reply.status, 200);
    assert_eq!(reply.body, IDENTITY);
    server.wait_for_log(|line| line["target"] == "game_lifecycle");
}

#[test]
fn without_log_dir_nothing_is_written_to_disk() {
    let scratch = Scratch::new("nolog");
    let server = Server::start_with(&[], Some(&scratch.0));

    get(server.addr, "/");
    post_json(
        server.addr,
        "/start",
        &request_json_with("v1.2.3", 2, 500, 0, &[]),
    );
    server.wait_for_log(|line| line["target"] == "game_lifecycle");

    let entries: Vec<_> = std::fs::read_dir(&scratch.0).unwrap().collect();
    assert!(entries.is_empty(), "{entries:?}");
}
