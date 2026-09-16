# Evidence: four-snake placement benchmark (003 T024, criterion 6)

## Environment identity

- Engine commit: `51a08c0` (the last code commit of feature 003 before this evidence; only documentation follows it), release profile, built with `cargo build --release --locked`.
- Host: AMD Ryzen 7 5700X (8 cores / 16 threads), 46 GiB, `Linux 7.2.5-3-omarchy x86_64`, Docker 29.7.2.
- Rules CLI: `.rules-oracle/battlesnake` v1.2.3, `battlesnake play -W 11 -H 11 -g standard -m standard -t 500`, seeds 1 to 30, four `--name/--url` pairs.
- Seats: the engine (`tiger`) or the sibling one-turn baseline (`baseline`) in seat one; Sansón (`sanson`, the previous engine, image `battle-snake:7212509`, the same image id as `battle-snake:current` on the VPS, run as a black box with `SANSON_DEEP=1 SANSON_REDUCE=1`), `flood-a` and `flood-b` (wrenger/snork at `76bec0c`, Flood agent) in seats two to four.
- Run date (UTC): 2026-09-19, 07:03 to 08:24 (80 minutes wall, 2 games at a time).
- Exact command: `target/release/spar --melee --output .specs/003-melee-search/evidence/melee-report.json --seeds 1-30 --workers 2 --overwrite --scratch target/sparring-melee`, roster `reference/roster-melee.json`.

## Fairness measures

- A first attempt with three games at a time was stopped after a minute: each Flood server runs its search on every core (about 7 cores each), the load average reached 25 on 16 threads and the engine got 66% of one core, which starves a time-bounded search. That run's games were discarded.
- The roster used for the evidence pins every engine to its own two physical cores (four logical CPUs, sibling pairs kept together): the engine and the baseline on CPUs 0, 1, 8, 9 via `taskset`; Sansón's container on 2, 3, 10, 11 via `--cpuset-cpus`; `flood-a` on 4, 5, 12, 13 and `flood-b` on 6, 7, 14, 15. With two games in flight the load average stayed near 10 and each Flood at about 390% CPU, the engine at 100 to 200%.
- The engine searches for 370 ms per move (its allowance), Sansón uses most of its 500 ms, the Floods answer in a few milliseconds; a four-snake game of the engine lasted 477 turns on average (the baseline's games 232), so the 60 games took 80 minutes.

## Result

```text
mean placement (1 best, 4 worst; shared eliminations averaged), 30 games each
tiger     2.300   (95% CI ±0.39; placements 1:9 2:8 3:8 4:5)
baseline  3.767   (95% CI ±0.20; never first)
sanson    3.267 in the tiger's games (95% CI ±0.28; never first)
          2.500 in the baseline's games (never first)
flood-a   2.333 beside the tiger, 1.867 beside the baseline
flood-b   2.100 beside the tiger, 1.867 beside the baseline
criterion 6: MET  (2.300 < 3.767 and 2.300 <= 3.267)
```

Per-game placements are in `melee-report.json` (contract `melee-report` 1.0.0, validated by the sparring tests).

## Reading

- The engine places a full 1.47 places better than the one-turn baseline and 0.97 places better than Sansón in the same 30 games; the intervals do not overlap for either comparison, so the bar (no worse than Sansón, clearly better than the baseline) is met with margin rather than by a hair.
- Sansón never won a game in this benchmark, in either seating; the two Floods are the strongest opponents here (they win most of the baseline's games between them). The engine won 9 of 30, more than any single opponent in its games (flood-b 8, flood-a 7, Sansón 0).
- Weaknesses to keep in view: 5 games ended with the engine eliminated first (placement 4), and its games are long (477 turns), which suggests cautious play that outlasts rather than kills; the head-to-head chances against shorter Floods may be under-used. These are the first things to try in a strength branch, with this benchmark as the yardstick.
- No weight was changed: the reasoned default profile (`DEFAULT_MELEE_PROFILE`) met the criterion on the first measured run, so it is the committed profile.
