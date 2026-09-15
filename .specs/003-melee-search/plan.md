# Plan - 003-melee-search

## Summary

A second, N-snake kernel (`arena::melee`, up to four serpents) proven equal to the reused resolver, a valuation pipeline for N snakes, and an iterative-deepening paranoid alpha-beta over serialized moves whose opponent layers only consider moves that do not kill their own snake. When two snakes remain the position is handed to the duel machinery of feature 001. A new `melee_search` route serves three and four snakes; everything else is unchanged. Sansón joins the sparring roster as a black-box opponent and the benchmark measures placement.

## Design

### Kernel (`arena::melee`)

- `MeleeBoard`: up to four `Serpent`s (the existing ring-buffer type) with an alive mask, `pellets`, `ply`; our seat is index 0. `advance(&self, moves: [Option<Heading>; 4]) -> MeleeAdvance` resolves all snakes at once in the Standard order (move, feed, eliminate) and returns the next board with dead snakes removed, and the placement facts of who was eliminated.
- Elimination rules for N snakes (docs.battlesnake.com/rules): health zero, off board, collision with any body (own included, tails that vacate excepted, exactly as the duel kernel), and head-to-head: the strictly longest head survives; equal longest heads all die; all others on that cell die. Simultaneous feeding by several heads on one pellet feeds each.
- Differential contract: for generated legal positions with 3 and 4 snakes and every joint move (4^N, sampled for N=4), `advance` equals `resolve_turn` on which snakes survive and their bodies, health and the food left, exactly like the duel kernel's test.
- `ingest_melee(&TurnState)` mirrors `ingest`; `MeleeBoard::as_duel()` converts a two-snake position to a `DuelBoard` (our seat first) for the hand-off.

### Move generation and search (`lookahead`)

- Layers per ply: our four headings, then each surviving opponent in turn (fixed seat order) choosing a heading, then `advance`. This is the paranoid (all opponents minimize our score) serialization of ADR 0003 extended to N.
- Self-preservation pruning: below the root ply, an opponent tries only headings that do not step off the board or into a body cell that will still be there (an opponent that must die whatever it does keeps all four). Our own layer and the opponents' root replies keep all four headings so certain-loss detection stays exact.
- Terminal scoring by placement: when we are eliminated with `k` opponents still alive the score is `-win + placement bonus`, so dying later is better; when only we remain it is `+win - ply penalty`; when exactly two snakes remain the leaf is valued by the duel pipeline through `as_duel()`.
- Iterative deepening, allowance, ordering (previous best, killers, history per seat) and the driver are the ones of feature 001, generalized behind a small `Position` trait rather than copied; duel behaviour and tests are unchanged.
- No transposition table (its gate from ADR 0005 is re-measured on melee positions only if profiling suggests it).

### Valuation (`valuation::melee`)

Assessors behind the same `Assessor` trait for N snakes, statically dispatched, integer weights in a `MeleeWeights` sheet:
1. Territory: multi-source time-aware fill (the duel `Fill` generalized to N sources), our cells minus the average of the opponents' or minus the best opponent's (decided by sparring).
2. Length: our length against the longest opponent and rank among the living.
3. Hunger and food reach: distance to the nearest pellet we reach first, with the health margin.
4. Head danger: adjacent-head threats from equal or longer snakes on the cells we could enter, and the cells where a shorter snake can be eaten.
5. Survival: number of opponents already eliminated on the line.
Initial weights are reasoned, not fitted; the benchmark drives changes.

### Application and transport

- `RouteSelector` gains `Route::MeleeSearch { state, board }` for three or four snakes; the duel and fallbacks are as before.
- `VerdictService` runs the melee search under the same allowance and falls back to the reused one-turn safety decision if no depth completes.
- `EnginePath::MeleeSearch` and `SelectionReason` reuse the existing reasons; the decision schema becomes 2.1.0 (additive `melee_search`).

### Sparring

- The transcript parser reads four-snake games and computes placements from the elimination order; the CLI runner plays four snakes; the roster gains a launcher for Sansón (`docker run` of the local image `battle-snake:7212509` on a free port, as a black box).
- The report contract gains a `placement` block (mean placement and its interval per snake role) in a new sparring-report version.

## Test strategy

- Kernel: unit examples for every rule (three heads on a cell, the longest survives, equal longest all die, tail vacating, simultaneous feeding), then the differential property against `resolve_turn` for 3 and 4 snakes.
- Search: alpha-beta equals an exhaustive paranoid minimax reference on generated positions at depths 1 to 2; never a certain loss when a survivor exists; interrupted iterations discarded; determinism with a fake clock.
- Valuation: per-assessor oracles like the duel ones (independent Dijkstra oracle for the N-source fill), bounds under the finite limit.
- Gateway: route table and real-router tests with three and four snakes; schema 2.1.0.
- Evidence: throughput and depth profile on melee positions, loopback latency at concurrency 16 with four-snake requests, branch coverage of the new modules, and the placement benchmark.

## Risks

- Branching: four snakes give 256 joint moves per ply; paranoid pruning plus opponent move pruning aim for depth 4 to 6 in 370 ms; the first profile decides whether a best-reply reduction is needed.
- Paranoid pessimism makes play timid in melee; sparring against Sansón is the check, and matrix or best-reply variants are follow-ups.
- Placement with four seats and 30 games is noisy; the evidence file will show intervals.
- Sansón's image is the operator's own previous work run as a black box; no source of it is read.
