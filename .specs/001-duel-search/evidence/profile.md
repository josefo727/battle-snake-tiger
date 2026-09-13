# Evidence: kernel throughput, search depth and position repeats (T030)

## Environment identity

- Commit: `57141c2861e4c4bafd792ff70c5b5a39b75cebfa` (clean tree; the harness at that commit produced every number below).
- Toolchain: `rustc 1.98.1 (48a229cea 2026-09-01) (Arch Linux rust 1:1.98.1-1)`, release profile.
- CPU: `AMD Ryzen 7 5700X 8-Core Processor`, 16 logical cores, 46 GiB, `Linux 7.2.5-3-omarchy x86_64`.
- Load average (1/5/15 min) at run time: `1.59 1.46 2.45` (default run) and `2.26 1.67 2.47` (sensitivity run); the desktop was otherwise idle, so timings carry a little noise; the repeat figures are deterministic.
- Run dates (UTC): `2026-09-19T00:15:49Z` and `2026-09-19T00:16:42Z` (19:15 and 19:16 on 2026-09-18, GMT-5).
- Exact commands:
  - `scripts/run-profile` (runs `cargo test --release --locked --test profile -- --ignored --nocapture --exact profile_the_search_core`)
  - `PROFILE_REPEAT_DEPTH_CAP=13 PROFILE_REPEAT_STRIDE=10 scripts/run-profile` (sensitivity check)

## What was measured

1. **Suite.** 200 midgame duels (ply 20 to 45) reproduced from seed `0x5eed20260918`: two coiled snakes on distinct start cells with 14 pellets, each steering toward the nearest pellet 60% of the time and never walking into certain death.
2. **Throughput.** Every joint move (16) of every position, 200 rounds (640,000 transitions), on the kernel `advance` and on the reused `resolve_turn`, on the same states; states and moves prepared outside the timed loops.
3. **Search.** Every position searched with the production settings (`deepen`, allowance ending 370 ms after the request's arrival, standard pipeline, learned move order).
4. **Repeats.** Every position replayed with a traced copy of the search (held to the real search's node count, value and heading by a test) to the depth it reached, capped. Positions are keyed by both bodies, both health values and the pellets, ignoring the ply. A visit is a *repeat* when the position was already seen in the same search; it is a *usable* repeat when the stored remaining depth is at least the visit's (what a depth-preferred table could answer). The gate figure is usable repeats over all visited nodes (terminal outcomes included) in the final fixed-depth iteration, i.e. one search.

## Result (default run)

```text
suite_size: 200                       suite_ply_min_max: 20 45
suite_mean_snake_length: 8.53         suite_positions_with_growth: 200
suite_mean_pellets_left: 2.94

kernel_ns_per_advance: 52.9           (18.9 million advances/s)
reference_ns_per_resolve: 139.5       (7.2 million resolves/s)
kernel_speedup: 2.6x

search_completed_depth_min_median_max: 1 12 15     (mean 11.11)
search_decisive_early_stops: 18
search_nodes_total: 135,505,940
search_nodes_per_second_aggregate: 2,929,407       (median position 2,935,354)
search_slowest_decision_ms: 370.3

repeat_depth_cap: 9   repeat_positions_sampled: 200
repeat_final_iteration_nodes: 7,089,326
repeat_final_iteration_raw_percent: 2.72
repeat_final_iteration_usable_percent: 2.72
repeat_whole_run_raw_percent: 8.26      (earlier iterations included)
repeat_whole_run_usable_percent: 2.07
repeat_usable_percent_median_position: 0.85
repeat_positions_at_or_above_gate: 6 of 200
```

## Sensitivity: deeper replays

The search reaches a median depth of 12 (maximum 15), while the default replay stops at depth 9 to bound the run. A second run replayed every tenth position (20 positions) up to depth 13:

```text
repeat_depth_cap: 13   repeat_positions_sampled: 20
repeat_final_iteration_nodes: 8,401,866
repeat_final_iteration_usable_percent: 8.28
repeat_whole_run_usable_percent: 7.27
repeat_usable_percent_median_position: 5.34
repeat_positions_at_or_above_gate: 4 of 20
```

## Transposition gate (ADR 0005, plan.md evidence gate): NOT PASSED

The gate needs at least 15% of visited positions to be repeats a table could answer. Measured: 2.72% at the depth-9 replay of all 200 positions and 8.28% at the depth-13 replay of 20 positions, both below 15%. The rate rises with depth (2.7% to 8.3%), so it is worth measuring again if a future change makes the search reach much deeper, but at the depths this engine reaches (median 12, maximum 15) it does not clear the bar. **T031 closes as skipped** with these rates.

## Findings beyond the gate

- The kernel's transition is only 2.6x faster than the reused, allocating resolver (52.9 against 139.5 ns), not the order of magnitude the plan hoped for; the by-value `DuelBoard` copy (two 128-slot rings) is the probable cost. A search node costs about 340 ns in total (2.93 million per second) including move ordering and the valuation pipeline, so the pipeline, not the transition, dominates. Any speed-up effort should profile the assessors first.
- 18 of 200 positions were solved early (a proven win or loss); one position completed only depth 1 (a forced, near-terminal position).
- The slowest decision took 370.3 ms of the 370 ms allowance: an iteration begun before 40% of the allowance is cut off at the deadline. (Correction, recorded with the T032 latency run: this does not make every decision use the whole allowance. When an iteration completes after the 40% mark the search ends there, so the median decision takes about 225-250 ms and the 95th percentile about 370 ms; see `latency.md`.)

## Limits of this evidence

- The suite's snakes are greedy-or-wandering, not strong players, and only 2.9 of 14 pellets are left on average; positions with more food or different shapes could repeat differently.
- The repeat replay is capped in depth (see the sensitivity run) and the traced search uses the standard pipeline and learned order, like production.
- Timings come from one machine that was not isolated from a desktop session.
