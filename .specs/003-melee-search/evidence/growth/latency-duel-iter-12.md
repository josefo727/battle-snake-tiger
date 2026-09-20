# Evidence: loopback deadline, latency and depth (T032)

## Environment identity

- Commit: `f5b867ca6d825dd122a09f78ae4db4eeb82df204`
- Toolchain: `rustc 1.98.1 (48a229cea 2026-09-01) (Arch Linux rust 1:1.98.1-1)`, release profile
- CPU: `AMD Ryzen 7 5700X 8-Core Processor`, 16 logical cores
- Kernel: `Linux 7.2.5-3-omarchy x86_64`
- Memory: `46Gi`
- Load average (1/5/15 min) at the end of the run: `8.24 4.72 3.79`
- Workload: warmup 200, concurrent 3000, sequential 100
- Exact command: `cargo test --release --locked --test latency -- --ignored --nocapture --exact worst_case_duels_meet_the_p99_deadline_on_loopback`
- Run date (UTC): `2026-09-20T04:24:17Z`

## Result

```text
=== latency report ===
declared_timeout_ms: 500
response_deadline_ms: 380
distinct_request_bodies: 16
warmup_requests: 200
concurrency: 16
concurrent_requests: 3000
concurrent_ok: 3000
concurrent_invalid_moves: 0
concurrent_p50_ms: 218.6
concurrent_p95_ms: 347.8
concurrent_p99_ms: 371.0
concurrent_max_ms: 373.6
concurrent_wall_s: 39.8
concurrent_decisions_per_second: 75.42
concurrent_cutoff_count: 0
concurrent_depth_min_median_max: 8 11 13
concurrent_depth_histogram: 8:2 9:326 10:635 11:1301 12:554 13:182
concurrent_nodes_total: 798496219
concurrent_nodes_per_second_wall: 20074515
concurrent_nodes_per_second_per_decision: 1265446
sequential_requests: 100
sequential_ok: 100
sequential_invalid_moves: 0
sequential_p50_ms: 249.3
sequential_p95_ms: 370.5
sequential_p99_ms: 370.6
sequential_max_ms: 370.6
sequential_wall_s: 25.3
sequential_decisions_per_second: 3.96
sequential_cutoff_count: 0
sequential_depth_min_median_max: 10 12 13
sequential_depth_histogram: 10:7 11:13 12:68 13:12
sequential_nodes_total: 73681200
sequential_nodes_per_second_wall: 2915888
sequential_nodes_per_second_per_decision: 2919065
=== end latency report ===
```

Gate: PASSED (p99 at or under declared_timeout - 120 ms, zero invalid moves, every duel decision searched to depth 2 or more, one decision event per request).
