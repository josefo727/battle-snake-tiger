# Evidence: loopback deadline, latency and depth (T032)

## Environment identity

- Commit: `fc909dda0b5a774665df99ba4e1848410de1b1c9`
- Toolchain: `rustc 1.98.1 (48a229cea 2026-09-01) (Arch Linux rust 1:1.98.1-1)`, release profile
- CPU: `AMD Ryzen 7 5700X 8-Core Processor`, 16 logical cores
- Kernel: `Linux 7.2.5-3-omarchy x86_64`
- Memory: `46Gi`
- Load average (1/5/15 min) at the end of the run: `7.73 11.33 6.99`
- Workload: warmup 1000, concurrent 20000, sequential 300
- Exact command: `cargo test --release --locked --test latency -- --ignored --nocapture --exact worst_case_duels_meet_the_p99_deadline_on_loopback`
- Run date (UTC): `2026-09-19T00:41:44Z`

## Result

```text
=== latency report ===
declared_timeout_ms: 500
response_deadline_ms: 380
distinct_request_bodies: 16
warmup_requests: 1000
concurrency: 16
concurrent_requests: 20000
concurrent_ok: 20000
concurrent_invalid_moves: 0
concurrent_p50_ms: 225.5
concurrent_p95_ms: 370.4
concurrent_p99_ms: 371.2
concurrent_max_ms: 377.5
concurrent_wall_s: 306.0
concurrent_decisions_per_second: 65.36
concurrent_cutoff_count: 0
concurrent_depth_min_median_max: 8 11 13
concurrent_depth_histogram: 8:11 9:1277 10:4944 11:8942 12:4825 13:1
concurrent_nodes_total: 5977141555
concurrent_nodes_per_second_wall: 19532392
concurrent_nodes_per_second_per_decision: 1223803
sequential_requests: 300
sequential_ok: 300
sequential_invalid_moves: 0
sequential_p50_ms: 250.4
sequential_p95_ms: 370.5
sequential_p99_ms: 370.6
sequential_max_ms: 370.6
sequential_wall_s: 72.0
sequential_decisions_per_second: 4.17
sequential_cutoff_count: 0
sequential_depth_min_median_max: 10 12 13
sequential_depth_histogram: 10:38 11:74 12:150 13:38
sequential_nodes_total: 209176673
sequential_nodes_per_second_wall: 2905000
sequential_nodes_per_second_per_decision: 2908347
=== end latency report ===
```

Gate: PASSED (p99 at or under declared_timeout - 120 ms, zero invalid moves, every duel decision searched to depth 2 or more, one decision event per request).
