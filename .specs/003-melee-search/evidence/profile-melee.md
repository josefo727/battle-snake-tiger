# Evidence: melee search depth and speed (003 T016)

## Environment identity

- Commit: `29c047bcb0001d67f2bee71594e97f3775555b88` (clean tree).
- Toolchain: `rustc 1.98.1 (48a229cea 2026-09-01) (Arch Linux rust 1:1.98.1-1)`, release profile.
- CPU: `AMD Ryzen 7 5700X 8-Core Processor`, 16 logical cores, 46 GiB, `Linux 7.2.5-3-omarchy x86_64`.
- Load average (1/5/15 min) at the four-snake run: `1.89 1.33 1.23`; the desktop was otherwise idle.
- Run date (UTC): `2026-09-19T06:43:18Z` (four snakes), a minute later for three.
- Exact commands:
  - `scripts/run-profile melee` (runs `cargo test --release --locked --test profile_melee -- --ignored --nocapture --exact profile_the_melee_search`)
  - `PROFILE_MELEE_SNAKES=3 scripts/run-profile melee`

## What was measured

1. **Suite.** 100 midgame melees (ply 20 to 45) reproduced from seed `0x5eed20260919`: four (or three) coiled snakes on distinct start cells with 14 pellets, each steering toward the nearest pellet 60% of the time and never walking into certain death, with some food left.
2. **Search.** Every position searched with the production settings (`deepen_melee`, allowance ending 370 ms after the request's arrival, standard melee valuation with the duel hand-off, learned move order, opponent self-preservation pruning below the root).
3. **Gate.** The median completed depth must be at least 3 (plan.md §Risks expected 4 to 6 for four snakes).

## Result

```text
four snakes (256 joint moves per ply)
search_completed_depth_min_median_max: 1 4 9     (mean 4.27)
search_decisive_early_stops: 5
search_nodes_total: 19,013,044
search_nodes_per_second_aggregate: 649,059
search_slowest_decision_ms: 371.7
depth_gate: median >= 3 -> PASSED

three snakes (64 joint moves per ply)
search_completed_depth_min_median_max: 1 6 12    (mean 6.57)
search_decisive_early_stops: 7
search_nodes_total: 22,668,565
search_nodes_per_second_aggregate: 837,139
search_slowest_decision_ms: 371.4
depth_gate: median >= 3 -> PASSED
```

## Reading

- Four snakes reach a median of 4 plies (every snake's next four moves), inside the 4 to 6 the plan expected; three snakes reach 6. The duel search reaches 12 on its own suite (`001-duel-search/evidence/profile.md`), so the melee is a shallower but still multi-turn search, as criterion 1 requires (at least one full extra turn for every snake is depth 2).
- Node rate is about a quarter of the duel's (649 k/s against 2.9 M/s): each node moves up to four serpents and a leaf runs the four-source fill. A minimum depth of 1 appears in a few crowded positions where depth 2 does not finish in the allowance.
- The slowest decision stays within 372 ms of the request's arrival, as the allowance intends.

## Decision

The gate passes with margin, so no further pruning (best-reply search, ADR 0007 Option C) is needed before the placement benchmark. The four-snake node rate is the first place to look if the benchmark asks for more depth.
