# Pause - 2026-09-20 (resume here)

## Where things are

- `main` = features 001 (duel search), 002 (game logging), 003 (melee search) plus growth iterations 4, 6, 8, 10, 11, 12 (kept). Full log with numbers: `.specs/003-melee-search/evidence/growth/growth-log.md`; journal sessions 44 to 47 in `JOURNAL.md`.
- Branch `improve/transposition-table`: the transposition table, complete and tested, measured on all three sources of gain and **not adopted** (no depth across turns, 4 of 12 duels against 5, light batch 1 of 8). Unmerged, kept for reference. The two profile harness switches (`PROFILE_TABLE`, `profile_turns.rs`) live only there.

## The yardstick (user's decision, 2026-09-19)

"Better than or equal to Shapeshifter" in four-snake games. Last full measure of `main` (30 seeds, Shapeshifter strong + two Flood, `reference/roster-shapeshifter-melee.json`): tiger 8 won, mean 2.62; Shapeshifter 8 won, mean 2.42. Light batch (`reference/roster-shapeshifter-light.json`, Shapeshifter + Mobility + one-turn, seeds 1-10): 3 won of 10, the duel finals against Shapeshifter go his way about 4 in 5. Duels against the strong build (`reference/roster-strong.json`, 12 seeds): 5 of 12.

## Open decision (ask the user, then go)

Which larger piece next, in the order proposed on 2026-09-20 night:
1. A post-mortem tool over the 12 duels against the strong build (turn where our reachable space falls behind for good, who initiates contact, lengths at 50/100, cause of death). 1-2 hours. Decides between 2 and 3.
2. Simultaneous-move treatment at the root of the duel search (ADR 0003 option B: the 4x4 matrix of our and their headings). 1-2 days.
3. Endgame valuation for duels: valuing the moment to cut (articulation points, parity), not only the separated state. 1-2 days.
4. A weight-tuning loop over the sparring harness at night (8 weights, 20-30 games per evaluation). Days of compute.
5. Time budget 370 to 395 ms. Cheap, marginal.
Use 60-seed batches before discarding small effects.

## Exact commands

- Verify: `scripts/verify`. Profiles: `scripts/run-profile [melee]`; latency: `scripts/run-latency [melee]` (`EVIDENCE_PATH=... LATENCY_REQUESTS=3000` for a short run).
- Full melee measure: `target/release/spar --melee --challenger-only --roster reference/roster-shapeshifter-melee.json --output <json> --seeds 1-30 --workers 2 --overwrite --scratch target/sparring-melee` (about 40 min).
- Light batch: same with `--roster reference/roster-shapeshifter-light.json --seeds 1-10` (about 15 min). Duels: `target/release/spar --roster reference/roster-strong.json --output <json> --seeds 1-12 --workers 2 --overwrite --scratch <dir>` (about 30 min, plays the baseline too).
- Always `cargo build --release --locked -p tiger-engine` before a measure (the rosters point at `target/release/tiger-engine`), never run two measures at once, and never compile while one runs.

## Habits that cost time this week

- `;` between `cargo test` and `git commit` let two green commits carry a failing test: use `&&`.
- A waiter loop with `pgrep -f "<text>"` matched its own command line and never exited: use `pgrep -f "spa[r] ..."` or `run_in_background` on the job.
- Integration tests run from `engine/`: pass absolute paths in `PROFILE_TRANSCRIPT`.
