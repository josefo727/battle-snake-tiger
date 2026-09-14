# Evidence: first sparring benchmark (T037)

## Environment identity

- Engine commit under test: `f76d01d54138efadae06660b81937b64f3568f76` (the commit that added the `spar` tool and provisioning; recorded inside the reports).
- Machine: AMD Ryzen 7 5700X (8 cores / 16 threads), `Linux 7.2.5-3-omarchy x86_64`, `rustc 1.98.1 (48a229cea 2026-09-01)`.
- Games run through the official rules CLI `v1.2.3` (executable digest pinned), standard 11x11, 500 ms timeout, food spawning left at the CLI's defaults.
- Dates: 2026-09-18 (about 20:27 to 21:38, GMT-5).
- Reports: `sparring-report.json` (criterion opponents) and `sparring-report-strong.json` (strong Shapeshifter), both validated against `contracts/sparring-report.schema.json` 1.0.0 by a test.
- Commands: `scripts/run-sparring --output .specs/001-duel-search/evidence/sparring-report.json --workers 4` and `scripts/run-sparring --roster reference/roster-strong.json --output .specs/001-duel-search/evidence/sparring-report-strong.json --seeds 1-12 --workers 2`.

## Opponents (black boxes, see PROVENANCE.md)

| id | what it is | how it is launched |
|----|------------|--------------------|
| `baseline` | the sibling one-turn safety engine (`rules-core`), the yardstick of criterion 8 | `BIND_ADDR=127.0.0.1 PORT=<port> target/baseline/release/rules-core` |
| `shapeshifter` | Shapeshifter from `../shapeshifter`, default Cargo features | `PORT=<port> reference/shapeshifter-default/release/shapeshifter` |
| `flood` | the `Flood` agent of wrenger/snork at commit `76bec0c` | `server --host 127.0.0.1:<port> --config '{"Flood":{}}'` |
| `shapeshifter-full` | Shapeshifter with `tt`, `parallel_search` and `mcts_fallback` (the README's `prod` set without `spl`, which does not build here) | `PORT=<port> reference/shapeshifter-full/release/shapeshifter` |

## Result 1: acceptance criterion 8 (seeds 1 to 30, 4 games at a time)

| Challenger | Opponent | Wins | Losses | Draws | Win rate | Wilson 95% | Mean turns |
|------------|----------|------|--------|-------|----------|------------|-----------|
| tiger | shapeshifter | 14 | 16 | 0 | 0.467 | [0.302, 0.639] | 454.9 |
| baseline | shapeshifter | 0 | 30 | 0 | 0.000 | [0.000, 0.114] | 47.7 |
| tiger | flood | 13 | 17 | 0 | 0.433 | [0.274, 0.608] | 406.1 |
| baseline | flood | 0 | 30 | 0 | 0.000 | [0.000, 0.114] | 67.3 |

- Against Shapeshifter (default): 0.467 against the baseline's 0.000, strictly greater.
- Against Flood: 0.433 against the baseline's 0.000, strictly greater.
- **Criterion 8 is met** as the specification states it.

## Result 2: the strongest Shapeshifter (seeds 1 to 12, 2 games at a time)

| Challenger | Opponent | Wins | Losses | Draws | Win rate | Wilson 95% | Mean turns |
|------------|----------|------|--------|-------|----------|------------|-----------|
| tiger | shapeshifter-full | 4 | 8 | 0 | 0.333 | [0.138, 0.609] | 422.9 |
| baseline | shapeshifter-full | 0 | 12 | 0 | 0.000 | [0.000, 0.242] | 43.2 |

## How to read this honestly

- **The bar of criterion 8 is low.** The one-turn baseline loses every game, in about 45 to 65 turns, so any engine that survives longer beats it. The informative figures are the win rates against the opponents themselves.
- **Where the engine stands.** Against Shapeshifter without its optional features and against Flood, tiger wins about 43 to 47% of 30 games: the Wilson intervals (roughly 0.27 to 0.64) include 0.5, so the data do not separate it from parity with either. Against the strongest Shapeshifter it won 4 of 12 (interval 0.14 to 0.61): behind, but not overwhelmed, and the sample is small.
- **Games are long** (400 to 450 turns on average against these three): both sides play for survival, so results are decided late and a sample of 30 is coarse.
- **Caveats on fairness.** Four (and, for the strong opponent, two) games ran at once on one machine, so every side had less CPU than in a single game; the engine's search is bounded by wall-clock time, so contention lowers its depth, and the strong Shapeshifter's multi-threaded search competes for the same cores. Flood's own default `--latency 100` subtracts 100 ms from its move budget. No opponent setting was tuned. The runs are not repeated, so run-to-run variation is unmeasured.
- **What this does not establish:** parity with Shapeshifter in production configuration (its `spl` feature did not build and the machine here is not its intended host), or any claim about other timeouts and board settings.

## Consequence for T038

The formal bar already holds after the first measurement, so the iteration task has no failing criterion to fix. Strength against the strong Shapeshifter is the open question; any further tuning would be a separate, measured effort under the same protocol.
