# Spec - 001-duel-search

## Summary

This feature gives the `tiger-king` identity a genuine multi-turn search and positional evaluation for Standard, non-wrapped 11x11 duels (exactly two active snakes), so it competes on strategy rather than only avoiding immediate death. Requests outside the duel case still receive a legal, deadline-safe move from the already-certified one-turn safety engine this project depends on.

## User story

As a competitive Battlesnake operator, I want my snake to look several turns ahead and weigh territory, space, health, and food in 1v1 duels, so that it can beat opponents that only avoid immediate death and compete meaningfully in tournament play.

## Acceptance criteria

1. For a valid `POST /move` request declaring Standard rules-module `v1.2.3` (or the local CLI's `cli` identifier), non-wrapped, hazard-free 11x11, exactly two active snakes, and a timeout greater than 120 ms, the engine selects its move by evaluating at least one full additional turn beyond the immediate one for both snakes, not only immediate survival.
2. The returned move is always one of the four platform-valid direction strings and the response completes within `game.timeout - 120 ms` of transport-recorded arrival, matching the sibling project's deadline contract.
3. When the allocated computation budget is exhausted before a deeper search layer completes, the engine returns the best move found at the deepest fully-completed search depth, never a partially evaluated or arbitrary move.
4. Each decision reports, without affecting the chosen move, the completed search depth and the number of positions explored.
5. A request outside the duel case (0, 1, 3, or 4 active snakes) or outside the certified ruleset/map/board scope receives the sibling project's existing one-turn safety or best-effort fallback response, unchanged from its own certified behavior.
6. `GET /` returns HTTP 200 with `Content-Type: application/json` and exactly `{ "apiversion": "1", "author": "josefo727", "color": "#00D5FF", "head": "tiger-king", "tail": "tiger-tail", "version": "0.1.0" }`.
7. Each valid `POST /start` and `POST /end` request returns HTTP 200 with `Content-Type: application/json` and an empty JSON object.
8. A fixed, reproducible sparring benchmark against [NEEDS CLARIFICATION: which declared external opponent(s) — Shapeshifter only, or also `snork`'s Flood?] over [NEEDS CLARIFICATION: how many games / what seed protocol counts as "statistically meaningful" for this project?] reports this engine's win rate, and that win rate is strictly greater than the sibling one-turn-safety-only engine's win rate under the identical benchmark.

## Non-goals

- Search or distinct strategic behavior for games with three or four active snakes (reserved for a later feature; such requests use the existing one-turn safety fallback unchanged).
- Board dimensions other than 11x11, wrapped maps, Royale, Squads, hazards, or custom rulesets.
- Rules-module versions other than `v1.2.3` (or the local CLI's `cli` identifier for that release).
- Adaptive opponent-risk modeling, phase-aware strategy switching, and per-move explainable telemetry beyond depth and node counts (candidate differentiators for a later feature).
- Parallel search, transposition tables, or other throughput optimizations not justified by measured profiling evidence.
- Training infrastructure, automated hyperparameter search, or online learning.
- Hosting-provider selection, deployment automation, or production registration on battlesnake.com.
- Source or behavioral compatibility with Shapeshifter or `snork`.

## Applicable constitution articles

- Article I - Every acceptance criterion and implementation change remains traceable to this specification.
- Article II - Each behavior is implemented through visible Red-Green-Refactor evidence.
- Article VII - The search and evaluation layers depend only on the declared Clock, RandomSource, diagnostics, RulesOracle, SparringRunner, and ReportSink boundaries.
- Article VIII - The search and evaluation design is independently derived and observably different in structure from any consulted reference engine.
- Article IX - The supported duel scope (Standard, non-wrapped, 11x11, exactly two snakes) is explicit and enforced at the transport boundary.
- Article X - Search and evaluation code depends inward only, reusing the sibling project's domain types without introducing transport, process, or filesystem dependencies.
- Article XI - The search preserves a deterministic legal fallback and respects the deadline under Article XI's controlled time.
- Article XII - Search and evaluation code meet the 90 percent branch-coverage floor with property-based invariant tests.
- Article XIV - Search depth, node throughput, and sparring results are observable and measured, not asserted.

## Open questions

- [NEEDS CLARIFICATION: which declared external opponent(s) for the sparring benchmark — Shapeshifter only, or also `snork`'s Flood?]
- [NEEDS CLARIFICATION: what game count / seed protocol counts as "statistically meaningful" sparring evidence for this project?]

## Glossary additions

- **Duel** - A supported request with exactly two active snakes, as distinct from a multiplayer (three or four snake) request.
- **Completed search depth** - The greatest number of full search plies evaluated to completion before the response deadline or a full-tree solve, whichever comes first.
- **Sparring benchmark** - A fixed, seeded set of repeated games against a declared external opponent, run through the official rules engine, whose aggregate results (win/loss/draw rate) are recorded as evidence rather than asserted.

---

## Closed (filled during verify)

- Date: `<YYYY-MM-DD>`
- Commit: `<sha>`
- Notes: `<non-obvious context for future readers>`
