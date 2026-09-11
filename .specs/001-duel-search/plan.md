# Plan - 001-duel-search

## Summary

`tiger-engine` answers Battlesnake webhooks like the sibling service, but for supported duels it replaces one-turn safety with a time-bounded, iterative-deepening alpha-beta search over a compact, allocation-free two-snake kernel, scored by a pipeline of independent assessors (time-aware Voronoi territory, sustenance, enclosure endgame, leverage, terminal scoring). Every non-duel request is delegated unchanged to the sibling's certified one-turn safety engine. The sibling rules core is reused as wire layer, fallback, clock port, and, importantly, as the reference model against which the new kernel is differentially tested. A separate `tiger-sparring` crate implements the offline `SparringRunner` and `ReportSink` boundaries and produces the reproducible Shapeshifter/Flood benchmark of spec criterion 8.

North star (project level, not a gate for this feature): reach and then exceed the measured strength of Shapeshifter across successive features. This feature's gate is criterion 8's relative bar; win rates against Shapeshifter and Flood are recorded every time so the trajectory is visible and never overstated.

## Stack decision

| Item | Choice | Rationale |
|------|--------|-----------|
| Language | Rust 1.98.1, edition 2024, pinned by `rust-toolchain.toml` | Identical to the reused core (ADR 0001); research §Web and runtime crates |
| Workspace | Cargo workspace: `engine`, `sparring` | ADR 0001; Article X enforced by the build graph |
| HTTP framework | Axum 0.8.9 on Tokio 1.53.1 | Same reviewed pins as the sibling (research §Web and runtime crates) |
| Serialization | Serde 1.0.229, serde_json 1.0.151 | Same pins; wire DTOs are reused from the sibling |
| Diagnostics | tracing 0.1.44, tracing-subscriber 0.3.23 | Same pins; schema v2 in `contracts/decision-diagnostic.schema.json` |
| Test runner | `cargo test --locked`, proptest 1.11.0, tower 0.5.3 (dev) | Same pins; property and in-process HTTP tests |
| Linter / types | `cargo clippy -D warnings`, `cargo fmt --check`, `unsafe_code = "forbid"` | Article X; rustc is the type checker |
| Dependency audit | cargo-deny 0.20.2 | Article XIII; `deny.toml` mirrors the sibling's reviewed policy |
| Coverage | cargo-llvm-cov 0.8.7 on nightly-2026-09-18, run in a disposable container | The sibling verified this exact approach (93.75% branch); Article XII |
| Rules reuse | `rules-core` path dependency at commit `efed780` | ADR 0002; `contracts/rules-core-dependency.rs` |
| Search | Iterative-deepening fail-soft alpha-beta over serialized moves | ADR 0003; research §Alpha-beta, §Iterative deepening |
| Evaluation | Statically dispatched assessor pipeline + integer `WeightSheet` | ADR 0004; research §Voronoi territory |
| Memoization | 64-bit Zobrist transposition table, adopted only if the measured gate passes | ADR 0005; research §Transposition tables |
| Runtime target | Linux amd64 release binary; digest-pinned multi-stage Docker image (same pinned base images as the sibling) | Sibling research §Docker official Rust and Debian images |

## Module layout

```
Cargo.toml                    # workspace
rust-toolchain.toml
deny.toml
engine/                       # package tiger-engine
  src/
    lib.rs                    # composition root: build_service(clock, beacon) -> Router
    main.rs                   # server binary; SystemClock; tracing subscriber
    arena/                    # compact game kernel, no allocation, no I/O
      cellset.rs              # CellSet(u128): 121 valid bits, shifts, edge masks, neighbours
      heading.rs              # Heading {North, East, South, West} + delta
      serpent.rs              # Serpent: ring-buffer body, vigor, length, cell set
      duel.rs                 # DuelBoard (two serpents + pellets); advance(a, b) -> Advance
      ingest.rs               # TurnState -> DuelBoard (the only bridge from the reused core)
    valuation/                # position assessment
      mod.rs                  # Assessor trait, Ledger, ValuationPipeline
      dominion.rs             # time-aware Voronoi territory
      sustenance.rs           # health margin vs. reachable food
      enclosure.rs            # separated-region survival estimate
      leverage.rs             # length difference + head-to-head pressure
      finish.rs               # terminal scoring
      weights.rs              # WeightSheet (integer coefficients, named profile)
    lookahead/                # search
      allowance.rs            # SearchAllowance: Clock-backed stop condition
      deepening.rs            # IterativeDeepening driver, completed-depth policy
      minimax.rs              # alpha-beta over Maximizer/Minimizer layers
      ordering.rs             # previous-best, killers per ply, history table
      memo.rs                 # Zobrist keys + TranspositionTable (gated)
      ledger.rs               # LookaheadReport (depth, nodes, best move, score)
    verdict/                  # application layer
      route.rs                # RouteSelector: Duel | SafetyFallback | UnsupportedFallback
      service.rs              # VerdictService pipeline; produces VerdictReport
    gateway/                  # transport
      http.rs                 # routes, content-type/size validation
      beacon.rs               # DecisionBeacon (diagnostic sink port) + tracing adapter
      settings.rs             # BIND_ADDR / PORT
  tests/
sparring/                     # package tiger-sparring (offline only)
  src/
    main.rs                   # `spar` binary
    runner.rs                 # SparringRunner port + official-CLI adapter
    roster.rs                 # opponent processes per contracts/opponent-roster.yaml
    transcript.rs             # game result parsing
    statistics.rs             # win rate, Wilson interval
    ledger.rs                 # ReportSink port + JSON file adapter
scripts/                      # verify, check-branch-coverage, run-latency, run-sparring
```

Entry points:
- `engine/src/main.rs` - the deployable server.
- `sparring/src/main.rs` - the offline benchmark tool.

Public interfaces:
- `VerdictService::decide(request, arrival) -> VerdictReport` - the only entry the transport calls.
- `Assessor::assess(&DuelBoard, &mut Ledger)` - one question per implementation.
- `SearchAllowance::should_stop()` - the only place the clock is consulted inside search.

## Data model

Entities (kernel, all `Copy` where practical):
- `CellSet(u128)`: bits 0..=120 valid (index = y * 11 + x); bits 121..=127 always zero.
- `Heading`: four values; `delta` is (dx, dy).
- `Serpent`: `ring: [u8; 128]` of cell indices, `head_slot: u8`, `length: u8`, `vigor: u8` (health), `cells: CellSet` (occupancy mirror of the ring). Tail = slot `head_slot - length + 1` (mod 128).
- `DuelBoard`: `serpents: [Serpent; 2]` (index 0 is us), `pellets: CellSet`, `ply: u16`.
- `Advance`: result of one joint move: `Continues(DuelBoard)` or `Over(Verdict)`, with `Verdict in {WeOnly, TheyOnly, BothDown}`.

Invariants:
- Each ring contains `length` distinct-or-stacked cells (a growth stack may repeat the tail cell); `cells` equals the union of the ring cells.
- Serpent cell sets may not intersect except for a stacked tail of the same serpent.
- `pellets` does not intersect any serpent cell set.
- `1 <= vigor <= 100` on live serpents; `length >= 1`; `length <= 121`.

Relationships: `ingest` builds a `DuelBoard` from `TurnState` and fails (typed `IngestError`) for anything but exactly two snakes; `advance` must equal `resolve_turn` from the reused core on every generated state and joint move (differential contract, task-level property test).

Migrations: none (no persistence).

## Routing and supported-scope predicate

`RouteSelector` reuses the sibling's `classify`:
1. `Scope::Supported(state)` with exactly two snakes and timeout above the reserve -> `DuelSearch` path.
2. `Scope::Supported(state)` with three or four snakes -> `SafetyFallback` (`decide_within_deadline`, unchanged).
3. `Scope::Unsupported(context)` -> `UnsupportedFallback` (`decide_unsupported`, unchanged).
4. If the duel path cannot complete depth 1 inside the allowance, the verdict falls back to `SafetyFallback` and records `budget_exhausted_before_first_depth`.

## Search and valuation

Search (ADR 0003):
- Deadline for search = sibling `response_deadline(arrival, timeout)` minus `SEARCH_TAIL_MARGIN` (10 ms, covering serialization and scheduling). The clock is polled every `POLL_INTERVAL_NODES` (1,024) visited nodes and at each iteration boundary.
- Iteration `d` runs only if elapsed time is below `ITERATION_START_FRACTION` (40%) of the search allowance; an interrupted iteration is discarded and depth `d - 1`'s result is returned.
- Tree: `Maximizer` layer over our four headings; for each, `Minimizer` layer over their four headings; then `advance`. Fail-soft alpha-beta at both layers. Terminal scores come from `finish` with a distance-to-end term so faster wins and slower losses are preferred.
- Ordering: previous iteration's best heading first; then per-ply killer headings; then a history table indexed by (side, heading); ties broken by fixed heading order (deterministic).
- Determinism: no randomness; identical input and allowance produce identical output (fake-clock testable).

Valuation (ADR 0004), integer arithmetic only:
- `Dominion`: multi-source layered fill on `CellSet`s. Layer `t` obstacles are all serpent cells except those released by turn `t` (segment `i` counted from the tail is free after `i + 1` turns, no growth assumed). Cells reached in the same layer by both are awarded to the longer serpent, and to nobody on equal length. Output: cells owned by us minus cells owned by them; a separate count of contested pellets we reach first.
- `Sustenance`: health margin against the distance to the nearest pellet we reach first, with urgency rising as the margin shrinks.
- `Enclosure`: active only when neither serpent's reachable region intersects the other's; estimates survival turns per region from its size, parity-adjusted cell counts, and tail release; output is our estimate minus theirs.
- `Leverage`: length difference and head-to-head pressure (our head adjacent to cells their head can enter while we are longer, and the reverse).
- `Finish`: forced-win / forced-loss / mutual-elimination scores with a ply-distance term, applied by search to terminal outcomes rather than registered in the positional pipeline (ADR 0004 amendment).
- `WeightSheet`: a named profile of integer coefficients (`DEFAULT_PROFILE`); changing weights never touches assessor code.

Design-diff notes (Article VIII, publicly documented reference shape only; reference internals were not consulted):

| Subsystem | Publicly documented reference shape | This project |
|-----------|-------------------------------------|--------------|
| Scope | One engine that also supports other board sizes and modes (feature flags) | Fixed 11x11, exactly two snakes in the search kernel; no size or player-count parameters |
| Position type | Bitboard-based, generic by mode | Flat `Copy` `DuelBoard` with ring-buffer bodies and a `u128` `CellSet` |
| Search | Minimax with memoization, optional parallel search and an MCTS fallback route | Duel-specialized two-layer alpha-beta; memoization gated by measurement; single-threaded here; no MCTS |
| Move endpoints | Two public move routes (minimax and MCTS) | One route; path chosen by `RouteSelector` |
| Evaluation | Configurable heuristics with tuned hyperparameters | Five single-question assessors behind one trait with a reportable ledger; weights tuned by reviewed sparring iterations |
| Composition | Handler-level calls into engine functions | Ports and a pipeline: `gateway -> verdict (RouteSelector -> VerdictService) -> lookahead -> valuation -> arena` |

## Boundaries

| Boundary | Adapter | Contract |
|----------|---------|----------|
| HTTP ingress (Battlesnake webhooks) | `gateway::http` | `contracts/openapi.yaml` |
| Rules core (reused) | `arena::ingest`, `verdict::route`, differential tests | `contracts/rules-core-dependency.rs` |
| Clock | reused `application::clock::Clock`; `SystemClock` in `main.rs`; fake in tests | n/a |
| Randomness | none in this feature (`RandomSource` unused) | n/a |
| Runtime diagnostics | `gateway::beacon::DecisionBeacon` + tracing adapter | `contracts/decision-diagnostic.schema.json` |
| External rule conformance | test-only adapter reusing the official CLI as in the sibling | sibling `contracts/rules-oracle.yaml` (referenced, unchanged) |
| Offline sparring | `sparring::runner::SparringRunner` | `contracts/opponent-roster.yaml` |
| Offline reports | `sparring::ledger::ReportSink` | `contracts/sparring-report.schema.json` |

The runtime move path has no database, queue, outbound HTTP, or filesystem access.

## Error model

- `IngestError` (not exactly two snakes, inconsistent state): never surfaced; `RouteSelector` treats it as "use the safety fallback".
- Search interruption: internal; the driver returns the last completed depth. If none completed, `budget_exhausted_before_first_depth` and the safety fallback.
- Transport: malformed JSON -> 400 without echoing the body; wrong or missing content type -> 415; body over 64 KiB -> 413; unsupported-but-valid requests still return 200 (same as the sibling).

## Observability

- One `move_decision` event per `/move` at INFO (WARN when a diagnostic other than `none` is present), schema 2.0.0: `engine_path`, `search_depth`, `nodes_explored`, `principal_score`, `elapsed_us`, `fallback_used`, `selected_move`, `selection_reason`, `diagnostic`. No board, bodies, names, or shouts.
- Correlation: `game_id` and `turn` on every event.
- Sparring reports are the offline metrics channel (`contracts/sparring-report.schema.json`).

## Security

- No authentication, money, PII, or external writes. Body-size and content-type limits mirror the sibling. No secrets exist. `unsafe_code` is forbidden. ADRs touched: none new.

## Test strategy

- Kernel: differential property tests of `advance` against the reused `resolve_turn` over generated legal duels and joint moves (movement, growth, stacked tails, starvation, boundaries, self/body collisions, head-to-head). This suite precedes every search task.
- Valuation: unit and property tests per assessor (symmetry: swapping sides negates the score; monotonicity in territory and length; integer bounds; enclosure only when regions are disjoint).
- Search: property "alpha-beta value equals exhaustive minimax value" at depths 1-3 on generated positions; legality of every returned move; completed-depth monotonicity; fake-clock tests for cutoff and "interrupted iteration is discarded"; determinism.
- Route/transport: in-process Axum tests with the real router for all four routes, every routing branch, malformed input, and diagnostics through a recording sink.
- Contract: compile-time import of every item in `contracts/rules-core-dependency.rs`; JSON Schema conformance for diagnostics and sparring reports.
- Conformance with the official engine: kernel-driven decisions replayed through the official CLI in an offline `#[ignore]`d suite, reusing the sibling's oracle pinned digest.
- Coverage: 90% branch on `arena`, `valuation`, and `lookahead`, measured in a disposable nightly container (Article XII); property suites for movement, growth, health, occupancy, elimination, termination, evaluation symmetry.
- Latency and depth: a release-mode loopback harness (same method as the sibling): 1,000 warmups then 20,000 requests at concurrency 16 on worst-case duels; records p50/p95/p99, max, cutoff count, invalid-move count, completed-depth distribution, nodes/second. Gate: p99 at or under `timeout - 120 ms`, zero invalid moves, completed depth of at least 2 on every duel decision. Depth and node throughput are recorded as strength indicators, not asserted beyond that floor.
- Sparring: `spar` runs 30 seeded games per opponent (seeds 1-30) for the challenger and for the sibling baseline; reports win rate with a 95% Wilson interval per matchup. Gate is spec criterion 8.

Evidence gate for the transposition table (ADR 0005): on a fixed suite of 200 generated midgame duels searched to the depth the baseline reaches within budget, if at least 15% of visited positions repeat within one search, the table task proceeds; otherwise it is recorded as skipped with the measured rate.

## Rollout

- Feature flag: none; first deployable behavior of this repository.
- Configuration: `BIND_ADDR` (default `0.0.0.0`), `PORT` (default `8080`), same as the sibling.
- Packaging: native release binary and a digest-pinned `linux/amd64` Docker image (same base digests as the sibling), non-root, read-only-root compatible, smoke-tested on all four routes.
- Rollback: redeploy the previous immutable image digest, or register the sibling's image (it implements the same identity).

## Roadmap after this feature (not commitments)

- 002: search and distinct strategy for three and four snakes.
- 003: parallel root split and transposition-table sizing, only with profiling evidence.
- 004: opponent-risk modeling and phase-aware weights, measured through the same sparring protocol.

## Risks

- Conservative serialization can play timidly; mitigated by measured review and the option to move to matrix-value refinement (ADR 0003).
- A second copy of the turn rules exists; mitigated by the differential suite and the contract on the reused core.
- Initial weights are reasoned, not fitted; mitigated by an iteration log of measured changes.
- Opponent builds are external; mitigated by liveness checks before any game and by recording opponent identity in every report.
