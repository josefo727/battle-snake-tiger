# Evidence: loopback deadline, latency and depth with four snakes (003 T017)

## Environment identity

- Commit: `f5b867ca6d825dd122a09f78ae4db4eeb82df204`
- Toolchain: `rustc 1.98.1 (48a229cea 2026-09-01) (Arch Linux rust 1:1.98.1-1)`, release profile
- CPU: `AMD Ryzen 7 5700X 8-Core Processor`, 16 logical cores
- Kernel: `Linux 7.2.5-3-omarchy x86_64`
- Memory: `46Gi`
- Load average (1/5/15 min) at the end of the run: `10.08 6.24 4.39`
- Workload: warmup 200, concurrent 3000, sequential 100
- Exact command: `cargo test --release --locked --test latency -- --ignored --nocapture --exact worst_case_melees_meet_the_p99_deadline_on_loopback`
- Run date (UTC): `2026-09-20T04:25:17Z`

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
concurrent_p50_ms: 126.1
concurrent_p95_ms: 371.8
concurrent_p99_ms: 372.6
concurrent_max_ms: 374.8
concurrent_wall_s: 34.4
concurrent_decisions_per_second: 87.23
concurrent_cutoff_count: 0
concurrent_depth_min_median_max: 3 4 5
concurrent_depth_histogram: 3:401 4:2411 5:188
concurrent_nodes_total: 247771590
concurrent_nodes_per_second_wall: 7204171
concurrent_nodes_per_second_per_decision: 454281
sequential_requests: 100
sequential_ok: 100
sequential_invalid_moves: 0
sequential_p50_ms: 240.8
sequential_p95_ms: 371.1
sequential_p99_ms: 371.2
sequential_max_ms: 371.4
sequential_wall_s: 22.4
sequential_decisions_per_second: 4.46
sequential_cutoff_count: 0
sequential_depth_min_median_max: 4 4 6
sequential_depth_histogram: 4:68 5:25 6:7
sequential_nodes_total: 24709186
sequential_nodes_per_second_wall: 1102376
sequential_nodes_per_second_per_decision: 1103646
=== end latency report ===
```

Gate: PASSED (p99 at or under declared_timeout - 120 ms, zero invalid moves, every melee decision searched to depth 1 or more (the histogram shows how many reached 2), one decision event per request).
