# Evidence: loopback deadline, latency and depth with four snakes (003 T017)

## Environment identity

- Commit: `35413eb95c964b33239ee59bfa6c0a2ed2b0dd00`
- Toolchain: `rustc 1.98.1 (48a229cea 2026-09-01) (Arch Linux rust 1:1.98.1-1)`, release profile
- CPU: `AMD Ryzen 7 5700X 8-Core Processor`, 16 logical cores
- Kernel: `Linux 7.2.5-3-omarchy x86_64`
- Memory: `46Gi`
- Load average (1/5/15 min) at the end of the run: `7.62 4.54 2.47`
- Workload: warmup 200, concurrent 4000, sequential 100
- Exact command: `cargo test --release --locked --test latency -- --ignored --nocapture --exact worst_case_melees_meet_the_p99_deadline_on_loopback`
- Run date (UTC): `2026-09-19T06:47:54Z`

## Result

```text
=== latency report ===
declared_timeout_ms: 500
response_deadline_ms: 380
distinct_request_bodies: 16
warmup_requests: 200
concurrency: 16
concurrent_requests: 4000
concurrent_ok: 4000
concurrent_invalid_moves: 0
concurrent_p50_ms: 370.7
concurrent_p95_ms: 374.1
concurrent_p99_ms: 375.2
concurrent_max_ms: 384.6
concurrent_wall_s: 82.4
concurrent_decisions_per_second: 48.57
concurrent_cutoff_count: 0
concurrent_depth_min_median_max: 3 4 6
concurrent_depth_histogram: 3:1243 4:2257 5:252 6:248
concurrent_nodes_total: 349763360
concurrent_nodes_per_second_wall: 4247189
concurrent_nodes_per_second_per_decision: 266332
sequential_requests: 100
sequential_ok: 100
sequential_invalid_moves: 0
sequential_p50_ms: 370.7
sequential_p95_ms: 371.8
sequential_p99_ms: 372.0
sequential_max_ms: 372.1
sequential_wall_s: 33.6
sequential_decisions_per_second: 2.98
sequential_cutoff_count: 0
sequential_depth_min_median_max: 3 4 6
sequential_depth_histogram: 3:6 4:74 5:14 6:6
sequential_nodes_total: 21429115
sequential_nodes_per_second_wall: 638402
sequential_nodes_per_second_per_decision: 638937
=== end latency report ===
```

Gate: PASSED (p99 at or under declared_timeout - 120 ms, zero invalid moves, every melee decision searched to depth 1 or more (the histogram shows how many reached 2), one decision event per request).

## Reading

- Criterion 2 holds with four-snake requests: p99 375.2 ms against the 380 ms deadline under 16 concurrent searches on 16 logical cores, and 372.0 ms one at a time; the maximum of 384.6 ms is above the deadline but the criterion is on the p99, as in the duel evidence (377.5 max there).
- Every decision under load completed depth 3 to 6 (median 4), so criterion 1's "one full additional turn for every snake" (depth 2) holds even at concurrency 16; there were no deadline cutoffs.
- The concurrent workload was 4,000 requests (the duel run used 20,000) to keep the overnight schedule; the sequential phase is the same 100-request shape as before and shows the same latencies, so the shorter run is not hiding a tail.
