//! Starts the compiled binary and talks to it over real loopback TCP.

mod support;

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::Value;

use support::request_json_with;
use support::server::{PATIENCE, Server, spawn};

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
            "safety_fallback",
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
