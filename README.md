# tiger-engine

An independently authored [Battlesnake](https://docs.battlesnake.com/) engine for
standard, non-wrapped 11x11 boards with two to four snakes. It plays on the platform
as **Sansón** (`tiger-king` identity) and is measured, not tuned by eye: every
change is adopted or rejected on a seeded sparring benchmark against black-box
opponents.

## What it does

- **Exact rules.** Turns are resolved by a compact, allocation-free kernel
  (`u128` cell sets, fixed-size `Copy` positions) that is proven equal to the
  conformance-verified rules core of the sibling `battle-snake-rust` project on
  generated states.
- **Deadline-aware search.** Iterative-deepening alpha-beta: a two-layer
  serialized tree for duels (ADR 0003), a paranoid tree with opponent pruning for
  three and four snakes (ADR 0007). A depth starts only if its predicted cost fits
  in the time left; an interrupted iteration is discarded and the last completed
  depth answers. Every request is answered about 130 ms before its declared
  timeout, with a one-turn safety fallback if no depth completes.
- **Composable valuation.** Small assessors (territory with time-aware tail
  release, sustenance, length, head pressure, enclosure; growth and finishing terms
  for melees) behind one trait, combined by a named weight sheet (ADR 0004).
- **Observable decisions.** Every move emits one JSON line with the path taken,
  depth, nodes, elapsed time and the contribution of each assessor; on a server
  they also go to daily log files.

## Layout

```
engine/       package tiger-engine: library tiger_engine + server binary
  src/arena       rules kernel (duel and melee boards, cell sets)
  src/lookahead   deepening, alpha-beta, paranoid search, ordering, allowance
  src/valuation   assessors and weight sheets (duel and melee)
  src/verdict     route selection and the decision service
  src/gateway     HTTP, settings, logging, lifecycle events
  tests/          integration, property, latency and profile harnesses
sparring/     package tiger-sparring: the offline benchmark tool `spar`
scripts/      verify, provision-opponents, run-sparring, run-profile, ...
.specs/       constitution, ADRs, feature specs, plans, tasks and evidence
```

Dependencies point inward: the engine never touches processes, files or the
network on its move path; the sparring tool lives in its own crate (ADR 0001).

## Build and run

Rust 1.98.1 is pinned by `rust-toolchain.toml`. The sibling rules core is a path
dependency: clone `battle-snake-rust` next to this repository.

```sh
cargo build --release --locked -p tiger-engine
BIND_ADDR=127.0.0.1 PORT=8080 target/release/tiger-engine
curl -s http://127.0.0.1:8080/
```

Settings (environment variables): `BIND_ADDR` (default `0.0.0.0`), `PORT`
(default `8080`), `LOG_DIR` (optional daily files `tiger.log.YYYY-MM-DD`),
`LOG_KEEP_DAYS` (default 14), `SEARCH_THREADS` (default 1; root splitting is
opt-in and was measured not to add depth).

Container image (the sibling is supplied as a named build context):

```sh
docker build --build-context sibling=../battle-snake-rust -t tiger-engine .
```

## Verify

```sh
scripts/verify            # fmt, clippy -D warnings, tests, architecture and coverage checks
scripts/run-profile       # duel search depth and node rate on a fixed suite
scripts/run-profile melee # the same for four snakes
scripts/run-latency       # p99 against the response deadline on loopback
```

## Sparring

Opponents are used only as black boxes over HTTP, driven by the official rules
CLI: Shapeshifter (two builds from the local checkout `../shapeshifter`) and
Flood (wrenger/snork, MIT). `scripts/provision-opponents` builds them into the
git-ignored `reference/` directory and writes the rosters; `scripts/fetch-rules-oracle`
fetches the CLI. Each seat is pinned to its own cores because the opponents search
on every core they can see.

```sh
# four-snake yardstick: Shapeshifter (strong build) + two Flood, 50 seeds, two games at a time
target/release/spar --melee --challenger-only \
  --roster reference/roster-shapeshifter-melee.json \
  --output report.json --seeds 1-50 --workers 2 --overwrite --scratch target/sparring-melee

# duels against the strong build
target/release/spar --roster reference/roster-strong.json \
  --output duels.json --seeds 1-50 --workers 2 --overwrite --scratch target/sparring-duel
```

Reports are versioned JSON with per-game placements and Wilson intervals; the
game transcripts stay in the scratch directory. The yardstick for adoption is
"not worse than Shapeshifter in four-snake games". As of growth iteration 15 the
engine is about even with it, not ahead: over three 50-seed runs of the same code
the tiger places 2.49 on average against Shapeshifter's 2.52, and finishes ahead of
it in 77 games and behind it in 72. Single runs spread widely (the first gave 2.40
against 2.62 and 31 ahead / 19 behind, a later one 2.49 against 2.33 and 21 / 29),
so one 50-game run does not separate changes smaller than about 0.3 places. In
duels the engine wins about 30% against the strong build (22 of 74), mostly losing
a race for length before being boxed in. The runs are in
`.specs/003-melee-search/evidence/growth/growth-log.md`.

## Method and provenance

Built with spec-driven development and test-first cycles (`.specs/`). No source, test, constant or structure of any reference engine
was used; `PROVENANCE.md` records every external source and the black-box role of
each opponent.
