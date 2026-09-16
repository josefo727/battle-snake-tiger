//! Deadline, latency and depth acceptance on real loopback TCP (criteria 2 and 4).
//!
//! The heavy run starts the compiled release binary, sends worst-case duels at
//! concurrency 16 and reads the depth and node counts from the server's own
//! decision log. It is `#[ignore]`d and driven by `scripts/run-latency`. The fast
//! tests here pin the harness (sampler, percentiles, log parsing, the gate) against
//! a stub server so its numbers can be trusted.

mod support;

use support::env_number;
use support::melee_suite::{
    MELEE_SUITE_SEED, MELEE_SUITE_SIZE, generate_melee_suite, melee_board_to_request_json,
};
use support::server::Server;
use support::suite::{SUITE_SEED, board_to_request_json, generate_suite};
use tiger_engine::arena::melee::MeleeBoard;
use tiger_engine::lookahead::paranoid::MeleeSearcher;
use tiger_engine::valuation::melee::MeleeValuation;
use tiger_engine::valuation::melee::finish::MeleeFinish;
use tiger_engine::valuation::melee::weights::DEFAULT_MELEE_PROFILE;

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::Value;
use tiger_engine::arena::duel::DuelBoard;
use tiger_engine::lookahead::minimax::Searcher;
use tiger_engine::valuation::StandardPipeline;
use tiger_engine::valuation::finish::Finish;
use tiger_engine::valuation::weights::DEFAULT_PROFILE;

const WARMUP_REQUESTS: usize = 1_000;
const CONCURRENT_REQUESTS: usize = 20_000;
/// Requests of the sequential comparison; each duel spends its whole allowance,
/// so 300 already take about two minutes.
const SEQUENTIAL_REQUESTS: usize = 300;
const CONCURRENCY: usize = 16;
const DECLARED_TIMEOUT_MS: u64 = 500;
const RESPONSE_RESERVE_MS: u64 = 120;
const MINIMUM_DUEL_DEPTH: u32 = 2;
/// A melee decision under sixteen concurrent searches must at least complete
/// its first depth; the histogram reports how many reach two or more.
const MINIMUM_MELEE_DEPTH: u32 = 1;

/// Which engine path a phase's decisions must take and how deep they must go.
#[derive(Clone, Copy)]
struct Expectation {
    path: &'static str,
    minimum_depth: u32,
}

const DUEL: Expectation = Expectation {
    path: "duel_search",
    minimum_depth: MINIMUM_DUEL_DEPTH,
};
const MELEE: Expectation = Expectation {
    path: "melee_search",
    minimum_depth: MINIMUM_MELEE_DEPTH,
};

// ---- the sampler ---------------------------------------------------------------------------

/// One client-observed request: how long it took over a fresh connection and
/// whether the answer was a 200 with a platform move.
struct RequestOutcome {
    latency: Duration,
    status_ok: bool,
    move_is_valid: bool,
}

/// Sends `count` requests over `concurrency` threads, cycling through `bodies`,
/// each on its own connection, and returns every outcome.
fn run_workload(
    addr: SocketAddr,
    bodies: &Arc<Vec<Vec<u8>>>,
    count: usize,
    concurrency: usize,
) -> Vec<RequestOutcome> {
    let next = AtomicUsize::new(0);
    let outcomes = Mutex::new(Vec::with_capacity(count));
    std::thread::scope(|scope| {
        for _ in 0..concurrency {
            scope.spawn(|| {
                loop {
                    let index = next.fetch_add(1, Ordering::SeqCst);
                    if index >= count {
                        break;
                    }
                    let outcome = send_move_request(addr, &bodies[index % bodies.len()]);
                    outcomes.lock().expect("never poisoned").push(outcome);
                }
            });
        }
    });
    outcomes.into_inner().expect("never poisoned")
}

/// One `/move` request over a brand-new connection with a minimal HTTP/1.1
/// client (the exchange is small and fixed). `Connection: close` keeps parsing to
/// "read to end". A connection-level failure is an outcome, not a crash: one
/// dropped connection in 20,000 must show up in the numbers.
fn send_move_request(addr: SocketAddr, body: &[u8]) -> RequestOutcome {
    let started = Instant::now();
    let (status_ok, move_is_valid) = try_send(addr, body).unwrap_or((false, false));
    RequestOutcome {
        latency: started.elapsed(),
        status_ok,
        move_is_valid,
    }
}

fn try_send(addr: SocketAddr, body: &[u8]) -> std::io::Result<(bool, bool)> {
    let mut stream = TcpStream::connect(addr)?;
    stream.set_nodelay(true)?;
    let head = format!(
        "POST /move HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes())?;
    stream.write_all(body)?;
    stream.flush()?;
    let mut response = Vec::new();
    stream.read_to_end(&mut response)?;

    let text = String::from_utf8_lossy(&response);
    let status_ok = text.starts_with("HTTP/1.1 200");
    let move_is_valid = status_ok
        && text
            .split_once("\r\n\r\n")
            .and_then(|(_, payload)| serde_json::from_str::<Value>(payload).ok())
            .and_then(|json| json["move"].as_str().map(str::to_owned))
            .is_some_and(|m| matches!(m.as_str(), "up" | "right" | "down" | "left"));
    Ok((status_ok, move_is_valid))
}

/// Nearest-rank percentile of an ascending list (zero for an empty one).
fn percentile(sorted: &[Duration], percent: f64) -> Duration {
    if sorted.is_empty() {
        return Duration::ZERO;
    }
    let rank = (percent / 100.0 * sorted.len() as f64).ceil() as usize;
    sorted[rank.clamp(1, sorted.len()) - 1]
}

// ---- reading the server's own account ---------------------------------------------------------

/// One decision as the server logged it.
#[derive(Debug, PartialEq)]
struct DecisionSample {
    engine_path: String,
    search_depth: u32,
    nodes_explored: u64,
    elapsed_us: u64,
    deadline_cutoff: bool,
}

/// The `move_decision` event inside one JSON log line, if that is what it is.
fn parse_decision_line(line: &str) -> Option<DecisionSample> {
    let outer: Value = serde_json::from_str(line).ok()?;
    if outer["target"] != "move_decision" {
        return None;
    }
    let event: Value = serde_json::from_str(outer["fields"]["message"].as_str()?).ok()?;
    Some(DecisionSample {
        engine_path: event["engine_path"].as_str()?.to_owned(),
        search_depth: u32::try_from(event["search_depth"].as_u64()?).ok()?,
        nodes_explored: event["nodes_explored"].as_u64()?,
        elapsed_us: event["elapsed_us"].as_u64()?,
        deadline_cutoff: event["diagnostic"] == "deadline_cutoff",
    })
}

#[derive(Debug, PartialEq)]
struct DepthSummary {
    min: u32,
    median: u32,
    max: u32,
    histogram: BTreeMap<u32, u64>,
}

fn summarize_depths(samples: &[DecisionSample]) -> DepthSummary {
    let mut depths: Vec<u32> = samples.iter().map(|s| s.search_depth).collect();
    depths.sort_unstable();
    let mut histogram = BTreeMap::new();
    for &depth in &depths {
        *histogram.entry(depth).or_insert(0) += 1;
    }
    DepthSummary {
        min: depths.first().copied().unwrap_or(0),
        median: depths.get(depths.len() / 2).copied().unwrap_or(0),
        max: depths.last().copied().unwrap_or(0),
        histogram,
    }
}

// ---- one phase and the gate -----------------------------------------------------------------------

struct PhaseReport {
    requests: usize,
    ok: usize,
    invalid_moves: usize,
    p50: Duration,
    p95: Duration,
    p99: Duration,
    max: Duration,
    wall: Duration,
    decisions: Vec<DecisionSample>,
}

fn deadline() -> Duration {
    Duration::from_millis(DECLARED_TIMEOUT_MS - RESPONSE_RESERVE_MS)
}

/// What is wrong with a phase against the deadline gate; empty when nothing is.
fn problems(phase: &PhaseReport) -> Vec<String> {
    problems_for(phase, DUEL)
}

fn problems_for(phase: &PhaseReport, expected: Expectation) -> Vec<String> {
    let mut found = Vec::new();
    if phase.p99 > deadline() {
        found.push(format!(
            "p99 {:?} is over the {:?} deadline",
            phase.p99,
            deadline()
        ));
    }
    if phase.invalid_moves > 0 {
        found.push(format!("{} invalid moves", phase.invalid_moves));
    }
    if phase.ok < phase.requests {
        found.push(format!(
            "status: {} of {} requests were not answered 200",
            phase.requests - phase.ok,
            phase.requests
        ));
    }
    let shallow = phase
        .decisions
        .iter()
        .filter(|d| d.search_depth < expected.minimum_depth)
        .count();
    if shallow > 0 {
        found.push(format!(
            "depth: {shallow} decisions completed fewer than {} plies",
            expected.minimum_depth
        ));
    }
    let fell_back = phase
        .decisions
        .iter()
        .filter(|d| d.engine_path != expected.path)
        .count();
    if fell_back > 0 {
        found.push(format!(
            "path: {fell_back} decisions were not made by {}",
            expected.path
        ));
    }
    if phase.decisions.len() != phase.requests {
        found.push(format!(
            "events: {} decision events for {} requests",
            phase.decisions.len(),
            phase.requests
        ));
    }
    found
}

// ---- a stub server -----------------------------------------------------------------------

/// A loopback server that answers every request the same way and remembers the
/// bodies it received.
struct Stub {
    addr: SocketAddr,
    handled: Arc<AtomicUsize>,
    bodies: Arc<Mutex<Vec<Vec<u8>>>>,
}

impl Stub {
    fn start(status: &'static str, body: &'static str, delay: Duration) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("a free port");
        let addr = listener.local_addr().unwrap();
        let handled = Arc::new(AtomicUsize::new(0));
        let bodies = Arc::new(Mutex::new(Vec::new()));
        let (h, b) = (handled.clone(), bodies.clone());
        std::thread::spawn(move || {
            for stream in listener.incoming().map_while(Result::ok) {
                let (h, b) = (h.clone(), b.clone());
                std::thread::spawn(move || serve(stream, status, body, delay, &h, &b));
            }
        });
        Self {
            addr,
            handled,
            bodies,
        }
    }

    fn handled(&self) -> usize {
        self.handled.load(Ordering::SeqCst)
    }
}

fn serve(
    mut stream: TcpStream,
    status: &str,
    body: &str,
    delay: Duration,
    handled: &AtomicUsize,
    bodies: &Mutex<Vec<Vec<u8>>>,
) {
    let mut request = Vec::new();
    let mut chunk = [0u8; 4096];
    let (head_end, length) = loop {
        let read = stream.read(&mut chunk).unwrap_or(0);
        if read == 0 {
            return;
        }
        request.extend_from_slice(&chunk[..read]);
        if let Some(end) = request.windows(4).position(|w| w == b"\r\n\r\n") {
            let head = String::from_utf8_lossy(&request[..end]).to_lowercase();
            let length = head
                .lines()
                .find_map(|l| {
                    l.strip_prefix("content-length:")
                        .map(|v| v.trim().parse::<usize>().unwrap_or(0))
                })
                .unwrap_or(0);
            break (end + 4, length);
        }
    };
    while request.len() < head_end + length {
        let read = stream.read(&mut chunk).unwrap_or(0);
        if read == 0 {
            break;
        }
        request.extend_from_slice(&chunk[..read]);
    }
    bodies.lock().unwrap().push(request[head_end..].to_vec());
    handled.fetch_add(1, Ordering::SeqCst);
    std::thread::sleep(delay);
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes());
}

fn bodies(n: usize) -> Arc<Vec<Vec<u8>>> {
    Arc::new(
        (0..n)
            .map(|i| format!("{{\"n\":{i}}}").into_bytes())
            .collect(),
    )
}

// ---- the sampler ---------------------------------------------------------------------------

#[test]
fn the_workload_issues_exactly_the_requested_number_of_requests() {
    let stub = Stub::start("200 OK", r#"{"move":"up"}"#, Duration::ZERO);

    let outcomes = run_workload(stub.addr, &bodies(3), 50, 4);

    assert_eq!(outcomes.len(), 50);
    assert_eq!(stub.handled(), 50);
    assert!(outcomes.iter().all(|o| o.status_ok && o.move_is_valid));
}

#[test]
fn the_workload_cycles_through_the_bodies_evenly() {
    let stub = Stub::start("200 OK", r#"{"move":"up"}"#, Duration::ZERO);

    run_workload(stub.addr, &bodies(4), 40, 3);

    let received = stub.bodies.lock().unwrap();
    for i in 0..4 {
        let wanted = format!("{{\"n\":{i}}}").into_bytes();
        assert_eq!(
            received.iter().filter(|b| **b == wanted).count(),
            10,
            "body {i}"
        );
    }
}

#[test]
fn latency_is_measured_across_the_whole_exchange() {
    let stub = Stub::start("200 OK", r#"{"move":"left"}"#, Duration::from_millis(40));

    let outcomes = run_workload(stub.addr, &bodies(1), 6, 2);

    assert_eq!(outcomes.len(), 6);
    assert!(
        outcomes
            .iter()
            .all(|o| o.latency >= Duration::from_millis(40))
    );
    assert!(outcomes.iter().all(|o| o.latency < Duration::from_secs(2)));
}

#[test]
fn a_bad_status_a_bad_move_and_a_dead_server_are_recorded_not_fatal() {
    let error = Stub::start("500 Internal Server Error", "{}", Duration::ZERO);
    let bad_move = Stub::start("200 OK", r#"{"move":"north"}"#, Duration::ZERO);
    let dead = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap();

    let errors = run_workload(error.addr, &bodies(1), 3, 1);
    let bad_moves = run_workload(bad_move.addr, &bodies(1), 3, 1);
    let refused = run_workload(dead, &bodies(1), 3, 1);

    assert!(errors.len() == 3 && errors.iter().all(|o| !o.status_ok && !o.move_is_valid));
    assert!(bad_moves.len() == 3 && bad_moves.iter().all(|o| o.status_ok && !o.move_is_valid));
    assert!(refused.len() == 3 && refused.iter().all(|o| !o.status_ok && !o.move_is_valid));
}

// ---- percentiles and depth -------------------------------------------------------------------

fn millis(values: impl IntoIterator<Item = u64>) -> Vec<Duration> {
    values.into_iter().map(Duration::from_millis).collect()
}

#[test]
fn percentiles_use_the_nearest_rank() {
    let hundred = millis(1..=100);

    assert_eq!(percentile(&hundred, 50.0), Duration::from_millis(50));
    assert_eq!(percentile(&hundred, 95.0), Duration::from_millis(95));
    assert_eq!(percentile(&hundred, 99.0), Duration::from_millis(99));
    assert_eq!(percentile(&hundred, 100.0), Duration::from_millis(100));
    assert_eq!(percentile(&millis([7]), 99.0), Duration::from_millis(7));
    assert_eq!(
        percentile(&millis([1, 2, 3, 4]), 50.0),
        Duration::from_millis(2)
    );
}

#[test]
fn a_decision_line_from_the_server_log_is_parsed() {
    let line = r#"{"timestamp":"2026-09-18T18:47:15.951302Z","level":"WARN","fields":{"message":"{\"level\":\"WARN\",\"target\":\"move_decision\",\"event\":\"move_decision\",\"schema_version\":\"2.0.0\",\"game_id\":\"g\",\"turn\":1,\"engine_path\":\"duel_search\",\"elapsed_us\":371234,\"search_depth\":11,\"nodes_explored\":987654,\"principal_score\":12,\"fallback_used\":false,\"selected_move\":\"up\",\"selection_reason\":\"search_completed_depth\",\"diagnostic\":\"deadline_cutoff\"}"},"target":"move_decision"}"#;

    let sample = parse_decision_line(line).expect("a decision line");

    assert_eq!(
        sample,
        DecisionSample {
            engine_path: "duel_search".to_owned(),
            search_depth: 11,
            nodes_explored: 987_654,
            elapsed_us: 371_234,
            deadline_cutoff: true,
        }
    );
}

#[test]
fn other_log_lines_are_not_decisions() {
    let listening = r#"{"timestamp":"t","level":"INFO","fields":{"message":"listening","addr":"127.0.0.1:1"},"target":"tiger_engine"}"#;

    assert_eq!(parse_decision_line(listening), None);
    assert_eq!(parse_decision_line("not json"), None);
    assert_eq!(parse_decision_line(""), None);
}

fn sample(depth: u32) -> DecisionSample {
    DecisionSample {
        engine_path: "duel_search".to_owned(),
        search_depth: depth,
        nodes_explored: 1_000,
        elapsed_us: 370_000,
        deadline_cutoff: false,
    }
}

#[test]
fn depth_summaries_count_every_depth() {
    let samples: Vec<_> = [9, 12, 12, 3, 15].into_iter().map(sample).collect();

    let summary = summarize_depths(&samples);

    assert_eq!((summary.min, summary.median, summary.max), (3, 12, 15));
    assert_eq!(
        summary.histogram,
        BTreeMap::from([(3, 1), (9, 1), (12, 2), (15, 1)])
    );
}

// ---- the gate --------------------------------------------------------------------------------------

fn healthy_phase() -> PhaseReport {
    PhaseReport {
        requests: 100,
        ok: 100,
        invalid_moves: 0,
        p50: Duration::from_millis(371),
        p95: Duration::from_millis(373),
        p99: Duration::from_millis(376),
        max: Duration::from_millis(379),
        wall: Duration::from_secs(10),
        decisions: (0..100).map(|_| sample(11)).collect(),
    }
}

#[test]
fn a_phase_inside_the_deadline_with_deep_valid_decisions_has_no_problems() {
    assert_eq!(problems(&healthy_phase()), Vec::<String>::new());
    assert_eq!(deadline(), Duration::from_millis(380));
}

#[test]
fn every_way_of_missing_the_gate_is_named() {
    type Breakage = Box<dyn Fn(&mut PhaseReport)>;
    let cases: Vec<(&str, Breakage)> = vec![
        ("p99", Box::new(|p| p.p99 = Duration::from_millis(381))),
        ("invalid", Box::new(|p| p.invalid_moves = 1)),
        ("status", Box::new(|p| p.ok = 99)),
        ("depth", Box::new(|p| p.decisions[5] = sample(1))),
        (
            "path",
            Box::new(|p| p.decisions[7].engine_path = "safety_fallback".to_owned()),
        ),
        ("events", Box::new(|p| p.decisions.truncate(90))),
    ];

    for (name, break_it) in cases {
        let mut phase = healthy_phase();
        break_it(&mut phase);

        let found = problems(&phase);

        assert_eq!(found.len(), 1, "{name}: {found:?}");
        assert!(found[0].contains(name), "{name}: {found:?}");
    }
}

#[test]
fn a_p99_exactly_at_the_deadline_passes() {
    let mut phase = healthy_phase();
    phase.p99 = Duration::from_millis(380);

    assert!(problems(&phase).is_empty());
}

// ---- the plan ---------------------------------------------------------------------------------------

#[test]
fn the_planned_workload_is_the_one_in_the_plan() {
    assert_eq!(WARMUP_REQUESTS, 1_000);
    assert_eq!(CONCURRENT_REQUESTS, 20_000);
    assert_eq!(CONCURRENCY, 16);
    assert_eq!(MINIMUM_DUEL_DEPTH, 2);
    assert_eq!(SEQUENTIAL_REQUESTS, 300);
}

// ---- measuring and reporting ------------------------------------------------------------------------

/// Sixteen evenly spaced suite positions the search cannot settle early: a
/// position solved within six plies would end its decision before the allowance
/// and say nothing about the deadline. What remains is the worst case, a search
/// that uses its whole allowance.
fn worst_case_bodies(suite: &[DuelBoard]) -> Vec<Vec<u8>> {
    let pipeline = StandardPipeline::standard();
    let unsettled: Vec<&DuelBoard> = suite
        .iter()
        .filter(|board| {
            let mut searcher = Searcher::new(&pipeline, Finish::new(&DEFAULT_PROFILE));
            let report = searcher.search_fixed(board, 6);
            !report
                .principal_score
                .is_some_and(|s| searcher.is_decisive(s))
        })
        .collect();
    let stride = (unsettled.len() / 16).max(1);
    unsettled
        .into_iter()
        .step_by(stride)
        .take(16)
        .map(|board| board_to_request_json(board).into_bytes())
        .collect()
}

/// The next `count` decision events from the server's log, or fewer if it goes
/// quiet for the server helper's patience.
fn collect_decisions(server: &Server, count: usize) -> Vec<DecisionSample> {
    let mut decisions = Vec::with_capacity(count);
    while decisions.len() < count {
        match server.logs.recv_timeout(support::server::PATIENCE) {
            Ok(line) => decisions.extend(parse_decision_line(&line)),
            Err(_) => break,
        }
    }
    decisions
}

fn measure_phase(
    server: &Server,
    bodies: &Arc<Vec<Vec<u8>>>,
    count: usize,
    concurrency: usize,
) -> PhaseReport {
    let started = Instant::now();
    let outcomes = run_workload(server.addr, bodies, count, concurrency);
    let wall = started.elapsed();
    let decisions = collect_decisions(server, count);

    let mut latencies: Vec<Duration> = outcomes.iter().map(|o| o.latency).collect();
    latencies.sort_unstable();
    PhaseReport {
        requests: count,
        ok: outcomes.iter().filter(|o| o.status_ok).count(),
        invalid_moves: outcomes.iter().filter(|o| !o.move_is_valid).count(),
        p50: percentile(&latencies, 50.0),
        p95: percentile(&latencies, 95.0),
        p99: percentile(&latencies, 99.0),
        max: latencies.last().copied().unwrap_or(Duration::ZERO),
        wall,
        decisions,
    }
}

fn ms(duration: Duration) -> String {
    format!("{:.1}", duration.as_secs_f64() * 1000.0)
}

fn render_phase(name: &str, phase: &PhaseReport) -> Vec<String> {
    let depth = summarize_depths(&phase.decisions);
    let histogram: Vec<String> = depth
        .histogram
        .iter()
        .map(|(d, n)| format!("{d}:{n}"))
        .collect();
    let nodes: u64 = phase.decisions.iter().map(|d| d.nodes_explored).sum();
    let busy_us: u64 = phase.decisions.iter().map(|d| d.elapsed_us).sum();
    let wall = phase.wall.as_secs_f64();
    vec![
        format!("{name}_requests: {}", phase.requests),
        format!("{name}_ok: {}", phase.ok),
        format!("{name}_invalid_moves: {}", phase.invalid_moves),
        format!("{name}_p50_ms: {}", ms(phase.p50)),
        format!("{name}_p95_ms: {}", ms(phase.p95)),
        format!("{name}_p99_ms: {}", ms(phase.p99)),
        format!("{name}_max_ms: {}", ms(phase.max)),
        format!("{name}_wall_s: {wall:.1}"),
        format!(
            "{name}_decisions_per_second: {:.2}",
            phase.requests as f64 / wall
        ),
        format!(
            "{name}_cutoff_count: {}",
            phase.decisions.iter().filter(|d| d.deadline_cutoff).count()
        ),
        format!(
            "{name}_depth_min_median_max: {} {} {}",
            depth.min, depth.median, depth.max
        ),
        format!("{name}_depth_histogram: {}", histogram.join(" ")),
        format!("{name}_nodes_total: {nodes}"),
        format!("{name}_nodes_per_second_wall: {:.0}", nodes as f64 / wall),
        format!(
            "{name}_nodes_per_second_per_decision: {:.0}",
            nodes as f64 * 1e6 / busy_us.max(1) as f64
        ),
    ]
}

/// The report the evidence file quotes.
fn render(
    bodies: usize,
    warmup: usize,
    concurrent: &PhaseReport,
    sequential: &PhaseReport,
) -> String {
    let mut lines = vec![
        "=== latency report ===".to_owned(),
        format!("declared_timeout_ms: {DECLARED_TIMEOUT_MS}"),
        format!("response_deadline_ms: {}", deadline().as_millis()),
        format!("distinct_request_bodies: {bodies}"),
        format!("warmup_requests: {warmup}"),
        format!("concurrency: {CONCURRENCY}"),
    ];
    lines.extend(render_phase("concurrent", concurrent));
    lines.extend(render_phase("sequential", sequential));
    lines.push("=== end latency report ===".to_owned());
    lines.join("\n")
}

/// Run by `scripts/run-latency` against the release binary: warmups, then the
/// concurrent phase, then a sequential comparison.
#[test]
#[ignore = "release-mode loopback run, driven by scripts/run-latency"]
fn worst_case_duels_meet_the_p99_deadline_on_loopback() {
    let warmup = env_number("LATENCY_WARMUP", WARMUP_REQUESTS);
    let requests = env_number("LATENCY_REQUESTS", CONCURRENT_REQUESTS);
    let sequential_requests = env_number("LATENCY_SEQUENTIAL", SEQUENTIAL_REQUESTS);

    let suite = generate_suite(SUITE_SEED);
    let bodies = Arc::new(worst_case_bodies(&suite));
    let server = Server::start();

    measure_phase(&server, &bodies, warmup, CONCURRENCY);
    let concurrent = measure_phase(&server, &bodies, requests, CONCURRENCY);
    let sequential = measure_phase(&server, &bodies, sequential_requests, 1);

    println!("{}", render(bodies.len(), warmup, &concurrent, &sequential));

    assert_eq!(
        problems(&concurrent),
        Vec::<String>::new(),
        "concurrent phase"
    );
    assert_eq!(
        problems(&sequential),
        Vec::<String>::new(),
        "sequential phase"
    );
}

/// Sixteen evenly spaced four-snake suite positions the search cannot settle
/// within three plies, so each decision uses its whole allowance.
fn worst_case_melee_bodies(suite: &[MeleeBoard]) -> Vec<Vec<u8>> {
    let valuation = MeleeValuation::standard();
    let unsettled: Vec<&MeleeBoard> = suite
        .iter()
        .filter(|board| {
            let mut searcher =
                MeleeSearcher::new(&valuation, MeleeFinish::new(&DEFAULT_MELEE_PROFILE));
            let report = searcher.search_fixed(board, 3);
            !report
                .principal_score
                .is_some_and(|s| searcher.is_decisive(s))
        })
        .collect();
    let stride = (unsettled.len() / 16).max(1);
    unsettled
        .into_iter()
        .step_by(stride)
        .take(16)
        .map(|board| melee_board_to_request_json(board).into_bytes())
        .collect()
}

/// Run by `scripts/run-latency melee` against the release binary with
/// four-snake requests (003 T017, criterion 2).
#[test]
#[ignore = "release-mode loopback run, driven by scripts/run-latency melee"]
fn worst_case_melees_meet_the_p99_deadline_on_loopback() {
    let warmup = env_number("LATENCY_WARMUP", WARMUP_REQUESTS);
    let requests = env_number("LATENCY_REQUESTS", CONCURRENT_REQUESTS);
    let sequential_requests = env_number("LATENCY_SEQUENTIAL", SEQUENTIAL_REQUESTS);

    let suite = generate_melee_suite(MELEE_SUITE_SEED, 4, MELEE_SUITE_SIZE);
    let bodies = Arc::new(worst_case_melee_bodies(&suite));
    let server = Server::start();

    measure_phase(&server, &bodies, warmup, CONCURRENCY);
    let concurrent = measure_phase(&server, &bodies, requests, CONCURRENCY);
    let sequential = measure_phase(&server, &bodies, sequential_requests, 1);

    println!("{}", render(bodies.len(), warmup, &concurrent, &sequential));

    assert_eq!(
        problems_for(&concurrent, MELEE),
        Vec::<String>::new(),
        "concurrent phase"
    );
    assert_eq!(
        problems_for(&sequential, MELEE),
        Vec::<String>::new(),
        "sequential phase"
    );
}

#[test]
fn the_melee_expectation_asks_for_the_melee_path_and_a_completed_depth() {
    let mut phase = healthy_phase();
    for decision in &mut phase.decisions {
        decision.engine_path = "melee_search".to_owned();
        decision.search_depth = 1;
    }

    assert_eq!(problems_for(&phase, MELEE), Vec::<String>::new());
    assert_eq!(problems_for(&phase, DUEL).len(), 2);
    phase.decisions[3].search_depth = 0;
    assert!(problems_for(&phase, MELEE)[0].contains("depth"));
}

#[test]
fn the_report_lists_every_measure_the_plan_asks_for() {
    let text = render(16, 1_000, &healthy_phase(), &healthy_phase());

    for wanted in [
        "concurrent_p50_ms: 371.0",
        "concurrent_p95_ms: 373.0",
        "concurrent_p99_ms: 376.0",
        "concurrent_max_ms: 379.0",
        "concurrent_cutoff_count: 0",
        "concurrent_depth_min_median_max: 11 11 11",
        "concurrent_depth_histogram: 11:100",
        "concurrent_nodes_per_second_wall: 10000",
        "sequential_invalid_moves: 0",
        "response_deadline_ms: 380",
    ] {
        assert!(text.contains(wanted), "missing `{wanted}` in\n{text}");
    }
}
